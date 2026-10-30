"""Isolated no-GUI acceptance of manual agents and five-source session import."""
import argparse
import http.server
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import threading
import time
import zstandard
from host_e2e import Rpc, wait


class Model(http.server.BaseHTTPRequestHandler):
    requests = []
    entered = threading.Event()
    release = threading.Event()
    def log_message(self, *_):
        pass
    def do_POST(self):
        value = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        Model.requests.append(value)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Connection', 'close')
        self.end_headers()
        if 'DEFERRED_CHILD' in json.dumps(value) and not Model.entered.is_set():
            Model.entered.set()
            Model.release.wait(30)
        if 'TODO_E2E' in json.dumps(value) and value.get('max_tokens',value.get('max_completion_tokens')) != 64:
            plans=[]
            for message in value.get('messages',[]):
                if message.get('role')!='tool':continue
                try:
                    data=json.loads(message.get('content',''))
                    if isinstance(data,dict) and isinstance(data.get('plan'),dict):plans.append(data['plan'])
                except (ValueError,TypeError):pass
            calls=[]
            if not plans:
                calls=[('plan','work_set_plan',{'expected_revision':0,'body':'Read then finish the synthetic checklist.','steps':['Read synthetic input','Finish synthetic work']})]
            else:
                plan=plans[-1]
                calls=[(f'progress-{i}','work_plan_progress',{'revision':plan['revision'],'step_id':step['id'],'status':'completed'}) for i,step in enumerate(plan['steps']) if step['status']!='completed']
            if calls:
                delta={'tool_calls':[{'index':i,'id':key,'type':'function','function':{'name':name,'arguments':json.dumps(args)}} for i,(key,name,args) in enumerate(calls)]}
                chunk={'id':'synthetic','choices':[{'index':0,'delta':delta,'finish_reason':'tool_calls'}]}
                self.wfile.write(('data: '+json.dumps(chunk)+'\n\ndata: [DONE]\n\n').encode());return
        chunk = {'id':'synthetic','choices':[{'index':0,'delta':{'content':'REDESIGN_OK'},'finish_reason':'stop'}]}
        self.wfile.write(('data: '+json.dumps(chunk)+'\n\ndata: [DONE]\n\n').encode())


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--bundle',type=Path,required=True)
    parser.add_argument('--report',type=Path,required=True)
    args=parser.parse_args()
    bundle=args.bundle.resolve()
    assert not (bundle/'expert_chat.exe').exists(), 'Use a deliberately GUI-free bundle'
    root=Path(tempfile.mkdtemp(prefix='expert-redesign-')).resolve()
    home=root/'home'; home.mkdir()
    fixtures={
        '.claude/projects/demo/session.jsonl': json.dumps({'type':'user','message':{'role':'user','content':'CLAUDE_IMPORT'}}),
        '.codex/sessions/2026/demo.jsonl': json.dumps({'type':'response_item','payload':{'type':'message','role':'user','content':[{'type':'input_text','text':'CODEX_IMPORT'}]}}),
        '.grok/sessions/demo/session.jsonl': json.dumps({'params':{'update':{'sessionUpdate':'user_message_chunk','content':{'type':'text','text':'GROK_IMPORT'}}}}),
    }
    for name,text in fixtures.items():
        path=home/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8')
    dsh=home/'.dsh/sessions/demo/session.v4.jsonl.zstd';dsh.parent.mkdir(parents=True)
    dsh.write_bytes(zstandard.ZstdCompressor().compress(json.dumps({'type':'user/message','data':{'id':'user','content':[{'type':'text','text':'DSH_IMPORT'}]}}).encode()))
    dbpath=home/'.local/share/opencode/opencode.db';dbpath.parent.mkdir(parents=True)
    db=sqlite3.connect(dbpath)
    db.executescript('CREATE TABLE session(id TEXT,title TEXT,directory TEXT,time_updated INTEGER,parent_id TEXT); CREATE TABLE message(id TEXT,session_id TEXT,data TEXT,time_created INTEGER); CREATE TABLE part(message_id TEXT,data TEXT,time_created INTEGER);')
    db.execute('INSERT INTO session VALUES(?,?,?,?,NULL)',('s','OpenCode import',str(root),int(time.time()*1000)))
    db.execute('INSERT INTO message VALUES(?,?,?,?)',('m','s',json.dumps({'role':'user'}),1))
    db.execute('INSERT INTO part VALUES(?,?,?)',('m',json.dumps({'type':'text','text':'OPENCODE_IMPORT'}),1));db.commit();db.close()
    project=root/'project';project.mkdir();(project/'AGENT.md').write_text('SYNTHETIC_AGENT_RULE: Read project files before proposing a patch.\n',encoding='utf-8')
    server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Model)
    threading.Thread(target=server.serve_forever,daemon=True).start()
    env={k:v for k,v in os.environ.items() if not k.startswith('EXPERT_')}
    env.update(USERPROFILE=str(home),HOME=str(home))
    log=(root/'host.log').open('wb')
    process=subprocess.Popen([str(bundle/'host/bin/expert-host.exe'),'--data-dir',str(root/'state'),'--workspace-cli',str(bundle/'expert-desktop.exe'),'--idle-seconds','0'],cwd=root,env=env,stdout=log,stderr=log,creationflags=subprocess.CREATE_NO_WINDOW)
    rpc=None
    checks=[]
    try:
        discovery=root/'state/host/discovery'
        wait(lambda:(discovery/'bridge.json').exists() or process.poll() is not None)
        assert process.poll() is None, f'Host startup failed: {root}'
        rpc=Rpc(discovery)
        workspace=rpc.call('workspace.register',{'path':str(project),'access':'read-write','confirmed':True})['workspace']
        prepared=rpc.call('workspace.prepare',{'path':str(project)})
        assert prepared['workspace']['id']==workspace['id'], prepared
        repeated=rpc.call('workspace.register',{'path':str(project),'access':'read-write','confirmed':True})['workspace']
        assert repeated['id']==workspace['id'], repeated
        assert len(rpc.call('workspace.list')['workspaces'])==1
        checks.append('reopening and registering one directory reuse its existing workspace ID')
        session=rpc.call('session.new',{'workspace_id':workspace['id']})['session_id']
        rpc.call('session.open',{'session_id':session})
        checks.append('workspace registration and session opening need no GUI executable')
        rpc.call('provider.save',{'name':'Synthetic','api_format':'openAiCompatible','auth_mode':'apiKey','api_key':'SYNTHETIC','base_url':f'http://127.0.0.1:{server.server_port}/v1','models':['gpt-5.1'],'chat_model':'gpt-5.1','enabled':True})
        choice=next(m['choice'] for m in rpc.call('models.list')['models'] if m['id']=='gpt-5.1')
        rpc.call('model.set',{'session_id':session,'choice':choice,'thinking':True,'effort':'high'})
        rpc.call('terminal.action',{'session_id':session,'action':'subagent.start','model':choice,'effort':'high','text':'DEFERRED_CHILD Reply REDESIGN_OK; no tools are needed.'})
        assert Model.entered.wait(15), 'First manual child did not start'
        def second_child():
            try:
                return rpc.call('terminal.action',{'session_id':session,'action':'subagent.start','model':choice,'effort':'high','text':'Second child: reply REDESIGN_OK; no tools are needed.'})
            except AssertionError as error:
                # Initial dispatch is serialized; a definite busy refusal has
                # no side effect and may be submitted after its setup finishes.
                if "'code': 'busy'" in str(error):
                    return None
                raise
        wait(second_child,5)
        children=wait(lambda: (lambda agents: agents if len(agents)>=2 else None)([a for a in rpc.call('agents.list',{'session_id':session})['agents'] if a.get('parent_id')]),15)
        assert len({a['parent_id'] for a in children})==1, children
        assert all(a['status']!='cancelled' for a in children), children
        Model.release.set()
        def child_done():
            agents=rpc.call('agents.list',{'session_id':session})['agents']
            return next((a for a in agents if a.get('parent_id') and a.get('status') in ('completed','failed')),None)
        child=wait(child_done,40)
        assert child['status']=='completed',child
        assert child['model']=='gpt-5.1',child
        assert 'REDESIGN_OK' in child.get('output',''),child
        wait(lambda:rpc.call('snapshot.get',{'session_id':session})['session']['status'] not in ('running','retrying','approval'),40)
        assert any('SYNTHETIC_AGENT_RULE' in json.dumps(r) for r in Model.requests), 'AGENT.md was not inherited'
        checks.append('manual subagent uses requested model, inherits AGENT.md, finishes and reports to parent')
        checks.append('a second manual child shares the parent and does not cancel the first')
        rpc.call('message.send',{'session_id':session,'text':'TODO_E2E: create the two-step synthetic plan, mark both completed using the progress tool, and reply REDESIGN_OK.'})
        def completed_plan():
            snapshot=rpc.call('snapshot.get',{'session_id':session})
            plan=snapshot.get('plan') or {}
            return snapshot if plan.get('steps') and all(s['status']=='completed' for s in plan['steps']) and snapshot['session']['status'] not in ('running','retrying','approval') else None
        completed=wait(completed_plan,30)
        assert completed['plan']['is_current']==True,completed['plan']
        checks.append('versioned todo progress reaches completed through real Host tool calls and snapshot updates')
        listing=rpc.call('terminal.query',{'session_id':session,'topic':'session.imports'})['sessions']
        assert {x['source'] for x in listing}=={'Claude Code','Codex','Grok Build','OpenCode','DeepSeek Harness'},listing
        before=len(Model.requests)
        for entry in listing:
            preview=rpc.call('terminal.query',{'session_id':session,'topic':'session.import.preview','id':entry['id']})
            imported=rpc.call('terminal.action',{'session_id':session,'action':'session.import','id':entry['id'],'fingerprint':preview['fingerprint'],'confirmed':True})['session_id']
            messages=rpc.call('snapshot.get',{'session_id':imported})['messages']
            assert any('_IMPORT' in m.get('text','') for m in messages)
        assert len(Model.requests)==before,'Import must not call a model'
        for name,text in fixtures.items():assert (home/name).read_text(encoding='utf-8')==text
        checks.append('all five source formats import into new sessions without modifying source or calling a model')
        forward=rpc.call('terminal.action',{'session_id':session,'action':'ssh.save','name':'Forward test','host':'127.0.0.1','port':22,'username':'unit','remote_forward':'10808 127.0.0.1:7890'})
        assert forward['remote_forward']=='10808 127.0.0.1:7890',forward
        read=rpc.call('terminal.query',{'session_id':session,'topic':'ssh.profile','id':forward['id']})
        assert read['remoteForwards'][0]['bindPort']==10808,read
        cleared=rpc.call('terminal.action',{'session_id':session,'action':'ssh.save','id':read['id'],'version':read['version'],'name':'Forward test','host':'127.0.0.1','port':22,'username':'unit','remote_forward':''})
        assert cleared['remote_forward']=='',cleared
        checks.append('optional RemoteForward saves, reloads and clears through the packaged Host without opening a network connection')
        rpc.call('host.shutdown',{'confirmed':True});process.wait(timeout=20)
        args.report.parent.mkdir(parents=True,exist_ok=True)
        args.report.write_text(json.dumps({'passed':True,'checks':checks,'paid_requests':0,'evidence_directory':str(root)},ensure_ascii=False,indent=2),encoding='utf-8')
        print(json.dumps({'passed':True,'checks':checks},ensure_ascii=False))
    finally:
        Model.release.set()
        if rpc:rpc.close()
        if process.poll() is None:process.terminate();process.wait(timeout=15)
        server.shutdown();server.server_close();log.close()
        print(f'Isolated evidence: {root}')


if __name__=='__main__':main()
