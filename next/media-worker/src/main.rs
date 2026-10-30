use expert_media_worker::{MAX_REQUEST, MEMORY_LIMIT, process_request};
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Write},
    time::Duration,
};

#[cfg(windows)]
fn limit_resources() -> Result<(), ()> {
    use std::{
        mem::{size_of, zeroed},
        ptr::null,
    };
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{JobObjects::*, Threading::GetCurrentProcess},
    };
    unsafe {
        let job = CreateJobObjectW(null(), null());
        if job.is_null() {
            return Err(());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        limits.BasicLimitInformation.ActiveProcessLimit = 1;
        limits.ProcessMemoryLimit = MEMORY_LIMIT as usize;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
            || AssignProcessToJobObject(job, GetCurrentProcess()) == 0
        {
            CloseHandle(job);
            return Err(());
        }
        // Keep the job handle until process exit: closing it would kill this worker.
    }
    Ok(())
}

#[cfg(unix)]
fn limit_resources() -> Result<(), ()> {
    unsafe {
        for (resource, limit) in [(libc::RLIMIT_AS, MEMORY_LIMIT), (libc::RLIMIT_CPU, 15)] {
            let value = libc::rlimit {
                rlim_cur: limit as libc::rlim_t,
                rlim_max: limit as libc::rlim_t,
            };
            if libc::setrlimit(resource, &value) != 0 {
                return Err(());
            }
        }
    }
    Ok(())
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 2 && args[1] == "--version" {
        println!("expert-media-worker {} (Rust)", env!("CARGO_PKG_VERSION"));
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(15));
        std::process::exit(124);
    });
    let result = std::panic::catch_unwind(|| {
        limit_resources().map_err(|_| "ResourceLimitUnavailable")?;
        if args.len() != 2 {
            return Err("InvalidRequest");
        }
        let file = File::open(&args[1]).map_err(|_| "InvalidRequest")?;
        let mut request = Vec::new();
        file.take((MAX_REQUEST + 1) as u64)
            .read_to_end(&mut request)
            .map_err(|_| "InvalidRequest")?;
        process_request(&request).map_err(|error| error.0)
    })
    .unwrap_or(Err("WorkerFailed"));
    let (value, mut code) = match result {
        Ok(value) => (value, 0),
        Err(error) => (json!({"ok": false, "error": error}), 1),
    };
    let payload = match serde_json::to_vec(&value) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST => bytes,
        _ => {
            code = 1;
            br#"{"ok":false,"error":"OutputTooLarge"}"#.to_vec()
        }
    };
    let mut output = std::io::stdout().lock();
    if output.write_all(&payload).is_err()
        || output.write_all(b"\n").is_err()
        || output.flush().is_err()
    {
        std::process::exit(1);
    }
    std::process::exit(code);
}
