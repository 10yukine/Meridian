use meridian_launcher::{Backend, Real, Setup};
use std::{path::PathBuf, time::Duration};

fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("pass a scratch game directory")?;
    let mut backend = Real::new(path);
    assert!(backend.sign_in_offline("bad name").is_err());
    assert_eq!(
        backend.sign_in_offline("A32_smoke")?,
        backend.account().unwrap()
    );
    backend.start_setup();
    let mut last = Setup::Needed;
    loop {
        let state = backend.setup();
        if state != last {
            println!("{state:?}");
            last = state;
        }
        match &last {
            Setup::Ready => return Ok(()),
            Setup::Failed { error, .. } => return Err(error.clone()),
            _ => std::thread::sleep(Duration::from_secs(1)),
        }
    }
}
