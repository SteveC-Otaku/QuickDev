//! Isolated Windows acceptance for the production capture/preserve factory.
//! Run: cargo run --offline --example retained_output_acceptance -- python
#[cfg(windows)]
#[path = "../src/process/drain.rs"]
mod output;

#[cfg(windows)]
fn main() {
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    if std::env::args().nth(1).as_deref() == Some("--qd-drain-logs") {
        output::drain();
        return;
    }

    // Own every spawned process, including cleanup after an assertion/panic.
    struct Fixture(Child);
    impl Drop for Fixture {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    fn wait(child: &mut Child) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.try_wait().expect("query fixture") {
                return status;
            }
            assert!(Instant::now() < deadline, "fixture exceeded 20 seconds");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let python = std::env::args().nth(1).expect("provide Python executable");
    let writer = "import sys;data='x'*8192+'\\n';[(sys.stdout.write(data),sys.stdout.flush(),sys.stderr.write(data),sys.stderr.flush()) for _ in range(256)]";
    let spawn_writer = || {
        Fixture(
            Command::new(&python)
                .args(["-c", writer])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x08000000)
                .spawn()
                .expect("spawn isolated writer"),
        )
    };

    let mut negative = spawn_writer();
    drop(negative.0.stdout.take());
    drop(negative.0.stderr.take());
    assert!(
        !wait(&mut negative.0).success(),
        "negative control must fail"
    );

    let mut source = spawn_writer();
    let retained = output::RetainedOutput::capture(&source.0).expect("capture both pipes");
    let mut helper = Fixture(retained.preserve().expect("handoff cloned pipe handles"));
    drop(source.0.stdout.take());
    drop(source.0.stderr.take());
    drop(retained);
    assert!(
        wait(&mut source.0).success(),
        "writer must survive parent reader closure"
    );
    assert!(wait(&mut helper.0).success(), "helper must exit at EOF");
    println!("PASS production capture/preserve: both original and retained parent handles closed; stdout/stderr each completed 2 MiB; helper exited at EOF");
}

#[cfg(not(windows))]
fn main() {
    panic!("Windows-only acceptance");
}
