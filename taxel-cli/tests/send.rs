use taxel_cli::{app, cmd};

#[test]
#[cfg_attr(feature = "external-test", ignore)]
fn test_send() {
    let args = vec![
        cmd::BIN,
        cmd::SEND,
        "--xml-file",
        "../test_data/taxonomy/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
    ];

    let app = app();
    let matches = app.get_matches_from(args);
    let subcommand_matches = matches.subcommand_matches(cmd::SEND).unwrap();
    println!("subcommand matches: {:#?}", subcommand_matches);

    let res = cmd::send(subcommand_matches);

    println!("res: {:#?}", res);
    assert!(res.is_ok())
}

#[test]
#[cfg_attr(feature = "external-test", ignore)]
fn test_send_and_print() {
    let args = vec![
        cmd::BIN,
        cmd::SEND,
        "--xml-file",
        "../test_data/taxonomy/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml",
        "--print",
        "ebilanz.pdf",
    ];

    let app = app();
    let matches = app.get_matches_from(args);
    let subcommand_matches = matches.subcommand_matches(cmd::SEND).unwrap();
    println!("subcommand matches: {:#?}", subcommand_matches);

    let res = cmd::send(subcommand_matches);

    println!("res: {:#?}", res);
    assert!(res.is_ok())
}
