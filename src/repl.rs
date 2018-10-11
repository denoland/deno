use rustyline::error::ReadlineError;
use rustyline::Editor;

use isolate;


#[allow(dead_code)]
pub fn repl_loop(isolate: &mut isolate::Isolate) {
    // `()` can be used when no completer is required
    let mut rl = Editor::<()>::new();
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_ref());
                isolate
                  .execute("deno repl", &line)
                  .unwrap_or_else(|err| {
                    println!("{}", err);
                  });
                // FIXME we should move this to a thread (and run it only once)...
                isolate.event_loop();
                // println!("Line: {}", line);
            },
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
    rl.save_history("history.txt").unwrap();
}