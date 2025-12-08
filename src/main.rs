const MAINTAINER_EMAIL: &str = "christoph.ungricht@outlook.com";

#[derive(Debug)]
enum Error {
    WrongNumberOfArguments(usize),
    ArgumentNotUTF8(std::ffi::OsString),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::WrongNumberOfArguments(num) => {
                if *num > 0 {
                    write!(
                        f,
                        "Wrong number of arguments. Expected 2 found {}.",
                        num - 1
                    )
                } else {
                    write!(
                        f,
                        "Somehow this OS does not pass the program name as the first argument. Contact the maintainer to fix the program for your OS: {MAINTAINER_EMAIL}."
                    )
                }
            }
            Error::ArgumentNotUTF8(argument) => {
                write!(f, "Weird argument found, not UTF-8: {argument:?}")
            }
        }
    }
}

fn cli() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().collect();
    let [_, from, to] = args
        .try_into()
        .map_err(|args: Vec<_>| Error::WrongNumberOfArguments(args.len()))?;

    let from = from.into_string().map_err(Error::ArgumentNotUTF8)?;
    let to = to.into_string().map_err(Error::ArgumentNotUTF8)?;
    println!("From {from} to {to}");
    Ok(())
}

fn main() -> std::process::ExitCode {
    if let Err(e) = cli() {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
