use taxel_cli::{app, cmd};

#[test]
fn test_validate() {
    let args = vec![
        cmd::BIN,
        cmd::VALIDATE,
        "--xml-file",
        "../test_data/instances/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
    ];

    let app = app();
    let matches = app.get_matches_from(args);
    let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
    println!("subcommand matches: {:#?}", subcommand_matches);

    let res = cmd::validate(subcommand_matches);

    println!("res: {:#?}", res);
    assert!(res.is_ok())
}

#[test]
fn test_validate_and_print() {
    let args = vec![
        cmd::BIN,
        cmd::VALIDATE,
        "--xml-file",
        "../test_data/instances/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
        "--print",
        "ebilanz.pdf",
    ];

    let app = app();
    let matches = app.get_matches_from(args);
    let subcommand_matches = matches.subcommand_matches(cmd::VALIDATE).unwrap();
    println!("subcommand matches: {:#?}", subcommand_matches);

    let res = cmd::validate(subcommand_matches);

    println!("res: {:#?}", res);
    assert!(res.is_ok())
}
