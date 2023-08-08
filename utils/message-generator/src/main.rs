mod executor;
mod external_commands;
mod into_static;
mod net;
mod parser;

#[macro_use]
extern crate load_file;

use crate::parser::sv2_messages::ReplaceField;
use binary_sv2::{Deserialize, Serialize};
use codec_sv2::{
    noise_sv2::formats::{EncodedEd25519PublicKey, EncodedEd25519SecretKey},
    StandardEitherFrame as EitherFrame,
};
use external_commands::*;
use roles_logic_sv2::parsers::AnyMessage;
use std::net::SocketAddr;
use arbitrary::Arbitrary;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Arbitrary)]
enum Sv2Type {
    Bool(bool),
    U8(u8),
    U16(u16),
    U24(Vec<u8>),
    U32(u32),
    U256(Vec<u8>),
    Str0255(String),
    B0255(Vec<u8>),
    B064K(Vec<u8>),
    B016m(Vec<u8>),
    B032(Vec<Vec<u8>>),
    Pubkey(Vec<u8>),
    Seq0255(Vec<Vec<u8>>),
    Seq064k(Vec<Vec<u8>>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SaveField {
    field_name: String,
    keyword: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
enum ActionResult {
    MatchMessageType(u8),
    MatchMessageField((String, String, Vec<(String, Sv2Type)>)),
    GetMessageField {
        subprotocol: String,
        message_type: String,
        fields: Vec<SaveField>,
    },
    MatchMessageLen(usize),
    MatchExtensionType(u16),
    CloseConnection,
    None,
}

impl std::fmt::Display for ActionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ActionResult::MatchMessageType(message_type) => {
                write!(
                    f,
                    "MatchMessageType: {} ({:#x})",
                    message_type, message_type
                )
            }
            ActionResult::MatchMessageField(message_field) => {
                write!(f, "MatchMessageField: {:?}", message_field)
            }
            ActionResult::MatchMessageLen(message_len) => {
                write!(f, "MatchMessageLen: {}", message_len)
            }
            ActionResult::MatchExtensionType(extension_type) => {
                write!(f, "MatchExtensionType: {}", extension_type)
            }
            ActionResult::CloseConnection => write!(f, "Close connection"),
            ActionResult::GetMessageField {
                subprotocol,
                fields,
                ..
            } => {
                write!(f, "GetMessageField: {:?} {:?}", subprotocol, fields)
            }
            ActionResult::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Copy)]
enum Role {
    Upstream,
    Downstream,
    Proxy,
}

#[derive(Debug, Clone)]
struct Upstream {
    addr: SocketAddr,
    /// If Some a noise connection is used, otherwise a plain connection is used.
    keys: Option<(EncodedEd25519PublicKey, EncodedEd25519SecretKey)>,
}

#[derive(Debug, Clone)]
struct Downstream {
    addr: SocketAddr,
    /// If Some a noise connection is used, otherwise a plain connection is used.
    key: Option<EncodedEd25519PublicKey>,
}

#[derive(Debug)]
pub struct Action<'a> {
    messages: Vec<(
        EitherFrame<AnyMessage<'a>>,
        AnyMessage<'a>,
        Vec<ReplaceField>,
    )>,
    result: Vec<ActionResult>,
    role: Role,
    actiondoc: Option<String>,
}

/// Represents a shell command to be executed on setup, after a connection is opened, or on
/// cleanup.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Command {
    command: String,
    args: Vec<String>,
    /// Stdout or Stderr conditions for when a command is considered a success or failure.
    conditions: ExternalCommandConditions,
}

/// Represents all of the parsed contents from the configuration file, ready for execution.
#[derive(Debug)]
pub struct Test<'a> {
    actions: Vec<Action<'a>>,
    /// Some if role is upstream or proxy.
    as_upstream: Option<Upstream>,
    /// Some if role is downstream or proxy.
    as_dowstream: Option<Downstream>,
    setup_commmands: Vec<Command>,
    execution_commands: Vec<Command>,
    cleanup_commmands: Vec<Command>,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let test_path = &args[1];
    println!();
    println!("EXECUTING {}", test_path);
    println!();
    let mut _test_path = args[1].clone();
    _test_path.insert_str(0, "../");
    let test_path_ = &_test_path;
    // Load contents of `test.json`, then parse
    let test = load_str!(test_path_);
    let test = parser::Parser::parse_test(test);
    let test_name: String = test_path
        .split("/")
        .collect::<Vec<&str>>()
        .last()
        .unwrap()
        .to_string();
    // Executes everything (the shell commands and actions)
    // If the `executor` returns false, the test fails
    let executor = executor::Executor::new(test, test_name).await;
    executor.execute().await;
    println!("TEST OK");
    std::process::exit(0);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::into_static::into_static;
    use roles_logic_sv2::{
        common_messages_sv2::{Protocol, SetupConnection},
        mining_sv2::{CloseChannel, SetTarget},
        parsers::{CommonMessages, Mining},
    };
    use std::{convert::TryInto, io::Write};
    use tokio::join;

    // The following test see that the composition serialise fist and deserialize
    // second is the identity function (on an example message)
    #[test]
    fn test_serialise_and_deserialize() {
        let message_string = r#"{"Mining":{"OpenExtendedMiningChannelSuccess":{"request_id":666666,"channel_id":1,"target":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255,255,255,255,255,255,255,255],"extranonce_size":16,"extranonce_prefix":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]}}}"#;
        let message_: AnyMessage<'_> = serde_json::from_str(&message_string).unwrap();
        let message_as_serde_value = serde_json::to_value(&message_).unwrap();
        let message_as_string = serde_json::to_string(&message_as_serde_value).unwrap();
        let message: AnyMessage<'_> = serde_json::from_str(&message_as_string).unwrap();
        let m_ = into_static(message);
        let message_as_string_ = serde_json::to_string(&m_).unwrap();

        let message_ = match message_ {
            AnyMessage::Mining(m) => m,
            _ => panic!(),
        };
        let message_ = match message_ {
            Mining::OpenExtendedMiningChannelSuccess(m) => m,
            _ => panic!(),
        };

        let m_ = match m_ {
            AnyMessage::Mining(m) => m,
            _ => panic!(),
        };
        let m_ = match m_ {
            Mining::OpenExtendedMiningChannelSuccess(m) => m,
            _ => panic!(),
        };
        if message_.request_id != m_.request_id {
            panic!();
        };
        if message_.channel_id != m_.channel_id {
            panic!();
        };
        if message_.target != m_.target {
            panic!();
        };
        if message_.extranonce_size != m_.extranonce_size {
            panic!();
        };
        if message_.extranonce_prefix != m_.extranonce_prefix {
            panic!();
        };
    }

    #[tokio::test]
    async fn it_send_and_receive() {
        let mut childs = vec![];
        let message = CloseChannel {
            channel_id: 78,
            reason_code: "no reason".to_string().try_into().unwrap(),
        };
        let frame = Sv2Frame::from_message(
            message.clone(),
            const_sv2::MESSAGE_TYPE_CLOSE_CHANNEL,
            0,
            true,
        )
        .unwrap();
        let server_socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 54254);
        let client_socket = SocketAddr::new("127.0.0.1".parse().unwrap(), 54254);
        let ((server_recv, server_send), (client_recv, client_send)) = join!(
            setup_as_upstream(server_socket, None, vec![], &mut childs),
            setup_as_downstream(client_socket, None)
        );
        server_send
            .send(frame.clone().try_into().unwrap())
            .await
            .unwrap();
        client_send
            .send(frame.clone().try_into().unwrap())
            .await
            .unwrap();
        let server_received = server_recv.recv().await.unwrap();
        let client_received = client_recv.recv().await.unwrap();
        match (server_received, client_received) {
            (EitherFrame::Sv2(mut frame1), EitherFrame::Sv2(mut frame2)) => {
                let mt1 = frame1.get_header().unwrap().msg_type();
                let mt2 = frame2.get_header().unwrap().msg_type();
                let p1 = frame1.payload();
                let p2 = frame2.payload();
                let message1: Mining = (mt1, p1).try_into().unwrap();
                let message2: Mining = (mt2, p2).try_into().unwrap();
                match (message1, message2) {
                    (Mining::CloseChannel(message1), Mining::CloseChannel(message2)) => {
                        assert!(message1.channel_id == message2.channel_id);
                        assert!(message2.channel_id == message.channel_id);
                        assert!(message1.reason_code == message2.reason_code);
                        assert!(message2.reason_code == message.reason_code);
                    }
                    _ => assert!(false),
                }
            }
            _ => assert!(false),
        }
    }

    #[test]
    fn it_create_tests_with_different_messages() {
        let message1 = CloseChannel {
            channel_id: 78,
            reason_code: "no reason".to_string().try_into().unwrap(),
        };
        let maximum_target: binary_sv2::U256 = [0; 32].try_into().unwrap();
        let message2 = SetTarget {
            channel_id: 78,
            maximum_target,
        };
        let message1 = Mining::CloseChannel(message1);
        let message2 = Mining::SetTarget(message2);
        let frame = Sv2Frame::from_message(
            message1.clone(),
            const_sv2::MESSAGE_TYPE_CLOSE_CHANNEL,
            0,
            true,
        )
        .unwrap();
        let frame = EitherFrame::Sv2(frame);
        let frame2 = Sv2Frame::from_message(
            message2.clone(),
            const_sv2::MESSAGE_TYPE_CLOSE_CHANNEL,
            0,
            true,
        )
        .unwrap();
        let frame2 = EitherFrame::Sv2(frame2);
        let _ = vec![frame, frame2];
        assert!(true)
    }

    #[tokio::test]
    async fn it_initialize_a_pool_and_connect_to_it() {
        //let mut bitcoind = os_command(
        //    "./test/bin/bitcoind",
        //    vec!["--regtest", "--datadir=./test/appdata/bitcoin_data/"],
        //    ExternalCommandConditions::new_with_timer_secs(10)
        //        .continue_if_std_out_have("sv2 thread start")
        //        .fail_if_anything_on_std_err(),
        //)
        //.await;
        //let mut child = os_command(
        //    "./test/bin/bitcoin-cli",
        //    vec![
        //        "--regtest",
        //        "--datadir=./test/appdata/bitcoin_data/",
        //        "generatetoaddress",
        //        "16",
        //        "bcrt1qttuwhmpa7a0ls5kr3ye6pjc24ng685jvdrksxx",
        //    ],
        //    ExternalCommandConditions::None,
        //)
        //.await;
        //child.unwrap().wait().await.unwrap();
        let mut pool = os_command(
            "cargo",
            vec![
                "llvm-cov",
                "--no-report",
                "run",
                "-p",
                "pool_sv2",
                "--",
                "-c",
                "./test/config/pool-config-sri-tp.toml",
            ],
            ExternalCommandConditions::new_with_timer_secs(60)
                .continue_if_std_out_have("Listening for encrypted connection on: 127.0.0.1:34254"),
        )
        .await;

        let setup_connection = CommonMessages::SetupConnection(SetupConnection {
            protocol: Protocol::MiningProtocol,
            min_version: 2,
            max_version: 2,
            flags: 0,
            endpoint_host: "".to_string().try_into().unwrap(),
            endpoint_port: 0,
            vendor: "".to_string().try_into().unwrap(),
            hardware_version: "".to_string().try_into().unwrap(),
            firmware: "".to_string().try_into().unwrap(),
            device_id: "".to_string().try_into().unwrap(),
        });

        let frame = Sv2Frame::from_message(
            setup_connection.clone(),
            const_sv2::MESSAGE_TYPE_SETUP_CONNECTION,
            0,
            true,
        )
        .unwrap();

        let frame = EitherFrame::Sv2(frame);

        let pool_address = SocketAddr::new("127.0.0.1".parse().unwrap(), 34254);
        let pub_key: EncodedEd25519PublicKey = "2di19GHYQnAZJmEpoUeP7C3Eg9TCcksHr23rZCC83dvUiZgiDL"
            .to_string()
            .try_into()
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let (recv_from_pool, send_to_pool) = setup_as_downstream(pool_address, Some(pub_key)).await;
        send_to_pool.send(frame.try_into().unwrap()).await.unwrap();
        match recv_from_pool.recv().await.unwrap() {
            EitherFrame::Sv2(a) => {
                assert!(true)
            }
            _ => assert!(false),
        }
        let mut child = os_command(
            "rm",
            vec!["-rf", "./test/appdata/bitcoin_data/regtest"],
            ExternalCommandConditions::None,
        )
        .await;
        child.unwrap().wait().await.unwrap();

        // TODO not panic in network utils but return an handler
        //pool.kill().unwrap();
        //bitcoind.kill().await.unwrap();
        assert!(true)
    }

    //#[tokio::test]
    //async fn it_test_against_remote_endpoint() {
    //    let proxy = match os_command(
    //        "cargo",
    //        vec![
    //            "run",
    //            "-p",
    //            "mining-proxy",
    //            "--",
    //            "-c",
    //            "./test/config/ant-pool-config.toml",
    //        ],
    //        ExternalCommandConditions::new_with_timer_secs(10)
    //            .continue_if_std_out_have("PROXY INITIALIZED")
    //            .warn_no_panic(),
    //    )
    //    .await
    //    {
    //        Some(child) => child,
    //        None => {
    //            write!(
    //                &mut std::io::stdout(),
    //                "WARNING: remote not avaiable it_test_against_remote_endpoint not executed"
    //            )
    //            .unwrap();
    //            return;
    //        }
    //    };
    //    //loop {}
    //    let _ = os_command(
    //        "cargo",
    //        vec!["run", "-p", "mining-device"],
    //        ExternalCommandConditions::new_with_timer_secs(10)
    //            .continue_if_std_out_have("channel opened with"),
    //    )
    //    .await;
    //    assert!(true)
    //}
}
