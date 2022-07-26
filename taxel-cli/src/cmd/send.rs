use clap::{Arg, ArgMatches};

pub fn send_args() -> [Arg<'static>; 0] {
    []
}

pub fn send(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send() {}
}
