use super::*;

/// How often the quest waiter checks whether the quest shell has exited.
pub(super) const QUEST_POLL_INTERVAL: Duration = Duration::from_millis(25);
/// How long a finished quest's remaining output may keep arriving.
pub(super) const QUEST_OUTPUT_GRACE: Duration = Duration::from_secs(1);

pub(super) fn run_quest(
    command: String,
    quest: u64,
    sender: mpsc::Sender<EngineCommand>,
    slot: Arc<Mutex<Option<Child>>>,
) {
    let refuse = |line: String| {
        let _ = sender.send(EngineCommand::QuestOutput {
            line,
            stderr: true,
            quest,
        });
        let _ = sender.send(EngineCommand::QuestDone {
            success: false,
            quest,
        });
    };
    let mut guard = match slot.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if guard.is_some() {
        refuse("A QUEST IS ALREADY RUNNING".into());
        return;
    }
    let Some(mut shell) = external_tools::quest_shell_command() else {
        refuse("FAILED TO START: INSTALL GIT FOR WINDOWS OR SET CQA_SHELL".into());
        return;
    };
    shell
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    external_tools::isolate_process_tree(&mut shell);
    let mut child = match shell.spawn() {
        Ok(child) => child,
        Err(error) => {
            refuse(format!("FAILED TO START: {error}"));
            return;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *guard = Some(child);
    drop(guard);

    // Cleared once the quest is reported done: a reader a surviving helper
    // keeps alive forwards nothing after that and exits on its next line.
    let forwarding = Arc::new(AtomicBool::new(true));
    let (drained_sender, drained) = mpsc::channel();
    let spawn_reader = |pipe: Option<Box<dyn std::io::Read + Send>>, stderr: bool| {
        let sender = sender.clone();
        let drained = drained_sender.clone();
        let forwarding = Arc::clone(&forwarding);
        thread::spawn(move || {
            if let Some(pipe) = pipe {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if !forwarding.load(Ordering::SeqCst) {
                        return;
                    }
                    let _ = sender.send(EngineCommand::QuestOutput {
                        line,
                        stderr,
                        quest,
                    });
                }
            }
            let _ = drained.send(());
        });
    };
    spawn_reader(stdout.map(|pipe| Box::new(pipe) as _), false);
    spawn_reader(stderr.map(|pipe| Box::new(pipe) as _), true);
    drop(drained_sender);
    thread::spawn(move || {
        let (status, mut process) = wait_for_quest_shell(&slot);
        // Output still in flight gets a short grace period. A helper the
        // finished quest left running could hold the pipes open forever, so
        // the quest's tree is stopped and its readers abandoned instead of
        // joined. On Windows `taskkill /T` cannot find the tree of a shell
        // that has already exited, so such a helper can outlive the quest;
        // `forwarding` and the quest generation keep its output out of any
        // later battle either way.
        let deadline = Instant::now() + QUEST_OUTPUT_GRACE;
        let drained_all = (0..2).all(|_| {
            drained
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .is_ok()
        });
        if let (false, Some(process)) = (drained_all, process.as_mut()) {
            external_tools::kill_process_tree(process);
        }
        forwarding.store(false, Ordering::SeqCst);
        let success = status.is_some_and(|status| status.success());
        let _ = sender.send(EngineCommand::QuestDone { success, quest });
    });
}

/// Waits for the quest shell itself (not its pipes) to exit and takes it out
/// of `slot`, so the next quest can start as soon as this one is over. The
/// slot stays locked only for each poll, so an abort can reach the shell.
pub(super) fn wait_for_quest_shell(
    slot: &Mutex<Option<Child>>,
) -> (Option<ExitStatus>, Option<Child>) {
    loop {
        {
            let Ok(mut guard) = slot.lock() else {
                return (None, None);
            };
            match guard.as_mut().map(Child::try_wait) {
                Some(Ok(None)) => {}
                Some(Ok(Some(status))) => return (Some(status), guard.take()),
                Some(Err(_)) | None => return (None, guard.take()),
            }
        }
        thread::sleep(QUEST_POLL_INTERVAL);
    }
}
