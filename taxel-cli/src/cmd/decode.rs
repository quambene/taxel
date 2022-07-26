use clap::{Arg, ArgMatches};

pub fn decode_args() -> [Arg<'static>; 0] {
    []
}

pub fn decode(matches: &ArgMatches) -> Result<(), anyhow::Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode() {
        todo!()
    }
}
