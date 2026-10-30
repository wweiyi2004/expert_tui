"""Encode the approved pose board into native terminal pixel frames.

Reads the approved raster without modifying it; writes only a JSON pixel atlas.
All poses use one scale and foot baseline. No anisotropic sprite stretching.
"""
from collections import Counter, deque
import json
from pathlib import Path
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'design/tui/pet/megumin-approved-poses.png'
DEST=ROOT/'design/tui/pet/megumin.json'
WIDTH,HEIGHT=64,80

def main():
    source=Image.open(SOURCE).convert('RGB')
    sw,sh=source.size
    cells=[]
    for row in range(2):
        for col in range(3):
            x0,x1=col*sw//3,(col+1)*sw//3
            y0,y1=row*sh//2,(row+1)*sh//2
            w,h=x1-x0,y1-y0
            background=set()
            queue=deque([(x,0) for x in range(w)]+[(x,h-1) for x in range(w)]+[(0,y) for y in range(h)]+[(w-1,y) for y in range(h)])
            while queue:
                x,y=queue.popleft()
                if (x,y) in background or not (0<=x<w and 0<=y<h):continue
                if min(source.getpixel((x+x0,y+y0)))<228:continue
                background.add((x,y))
                queue.extend(((x-1,y),(x+1,y),(x,y-1),(x,y+1)))
            remaining={(x,y) for y in range(h) for x in range(w) if (x,y) not in background}
            components=[]
            while remaining:
                seed=remaining.pop(); component={seed}; queue=deque([seed])
                while queue:
                    x,y=queue.popleft()
                    for neighbor in ((x-1,y),(x+1,y),(x,y-1),(x,y+1)):
                        if neighbor in remaining:
                            remaining.remove(neighbor);component.add(neighbor);queue.append(neighbor)
                components.append(component)
            mask=max(components,key=len)
            ink=list(mask)
            left,top,right,bottom=min(x for x,y in ink),min(y for x,y in ink),max(x for x,y in ink),max(y for x,y in ink)
            feet=[x for x,y in ink if y>=bottom-10]
            anchor=sum(feet)/len(feet) if row==0 or col<2 else (left+right)/2
            cells.append((x0,y0,w,h,mask,(left,top,right,bottom),anchor))
    scale=(HEIGHT-8)/max(c[5][3]-c[5][1]+1 for c in cells[:5])
    raw=[]
    for x0,y0,w,h,mask,bounds,anchor in cells:
        # Keep the entire silhouette in frame, including the staff tip.
        anchor=max(bounds[2]-(WIDTH/2-2)/scale,
                   min(anchor,bounds[0]+(WIDTH/2-2)/scale))
        baseline=bounds[3]
        frame=[]
        for y in range(HEIGHT):
            line=[]
            for x in range(WIDTH):
                sx=round(anchor+(x-WIDTH/2)/scale)
                sy=round(baseline+(y-(HEIGHT-3))/scale)
                line.append(source.getpixel((x0+sx,y0+sy)) if (sx,sy) in mask else None)
            frame.append(line)
        raw.append(frame)
    hist=Counter(tuple(round(c/16)*16 if c<248 else 255 for c in rgb) for f in raw for line in f for rgb in line if rgb)
    colors=[(40,16,20),(64,24,24),(96,40,24),(144,64,32),(160,32,40),(208,48,48),(248,80,48),(240,160,64),(255,208,112),(248,192,160),(255,224,192),(255,248,224)]
    colors=list(dict.fromkeys(colors+[rgb for rgb,_ in hist.most_common(36)]))
    symbols='0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'
    palette={symbols[i]:'#%02x%02x%02x'%rgb for i,rgb in enumerate(colors)}
    def encode(frame):
        return [''.join('.' if rgb is None else symbols[min(range(len(colors)),key=lambda i:sum((rgb[k]-colors[i][k])**2 for k in range(3)))] for rgb in line) for line in frame]
    def shift(frame,dy):
        return [frame[y-dy][:] if 0<=y-dy<HEIGHT else [None]*WIDTH for y in range(HEIGHT)]
    named={'idle':raw[0],'idle_breath':shift(raw[0],-1),'blink':raw[1],'pose':raw[2],
           'pose_closed':raw[0],'chant1':raw[3],'chant2':shift(raw[3],-1),
           'boom1':raw[4],'boom2':shift(raw[4],-1),'boom3':raw[4],
           'down':raw[5]}
    DEST.write_text(json.dumps({'width':WIDTH,'height':HEIGHT,'source':'megumin-approved-poses.png','palette':palette,'frames':{k:encode(v) for k,v in named.items()}},ensure_ascii=False,indent=1)+'\n',encoding='utf-8')
    print(json.dumps({'width':WIDTH,'height':HEIGHT,'palette':len(palette),'frames':list(named),'source_bounds':[c[5] for c in cells]}))

if __name__=='__main__':main()
