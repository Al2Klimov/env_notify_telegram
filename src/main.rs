mod cli;

use cli::EnvError;
use std::process::exit;

fn main() {
    match cli::parse_env() {
        Err(err) => {
            eprint!("Environment variable {} ", err.var);

            match err.err {
                EnvError::Missing => eprintln!("missing"),
                EnvError::Empty => eprintln!("is empty"),
                EnvError::BadUnicode(err) => eprintln!("is not valid UTF-8: {}", err),
            }

            exit(3);
        }
        Ok(action) => {
            todo!()
        }
    }
}
