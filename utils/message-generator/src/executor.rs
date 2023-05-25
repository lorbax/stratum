use crate::{
    external_commands::os_command,
    net::{setup_as_downstream, setup_as_upstream},
    Action, ActionResult, Command, Role, Sv2Type, Test,
};
use codec_sv2::{buffer_sv2::Slice, StandardEitherFrame};

use async_channel::{Receiver, Sender};
use codec_sv2::{Frame, StandardEitherFrame as EitherFrame, Sv2Frame};
use roles_logic_sv2::{parsers::{AnyMessage, PoolMessages, CommonMessages, IsSv2Message, self}, common_messages_sv2::{SetupConnection, ChannelEndpointChanged, SetupConnectionError, SetupConnectionSuccess}, mining_sv2::{CloseChannel, NewExtendedMiningJob, NewMiningJob, OpenExtendedMiningChannel, OpenExtendedMiningChannelSuccess, OpenMiningChannelError, OpenStandardMiningChannel, OpenStandardMiningChannelSuccess, Reconnect, SetCustomMiningJob, SetCustomMiningJobError, SetCustomMiningJobSuccess, SetExtranoncePrefix, SetGroupChannel, SetNewPrevHash as MiningSetNewPrevHash, SetTarget, SubmitSharesError, SubmitSharesExtended, SubmitSharesStandard, SubmitSharesSuccess, UpdateChannel, UpdateChannelError}, job_negotiation_sv2::{AllocateMiningJobToken, AllocateMiningJobTokenSuccess, CommitMiningJob, CommitMiningJobSuccess}, template_distribution_sv2::{CoinbaseOutputDataSize, NewTemplate, RequestTransactionData, RequestTransactionDataError, RequestTransactionDataSuccess, SetNewPrevHash, SubmitSolution}};
use std::convert::TryInto;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::any::type_name;
use serde_json::{json};
use crate::into_static::into_static;

//use crate::TestMessageParser;

use std::time::Duration;
use tokio::{fs::File, io::{copy, BufWriter, BufReader}, time::timeout};
//use roles_logic_sv2::mining_sv2::open_channel::OpenExtendedMiningChannel;
fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

pub struct Executor {
    name: Arc<String>,
    send_to_down: Option<Sender<EitherFrame<AnyMessage<'static>>>>,
    recv_from_down: Option<Receiver<EitherFrame<AnyMessage<'static>>>>,
    send_to_up: Option<Sender<EitherFrame<AnyMessage<'static>>>>,
    recv_from_up: Option<Receiver<EitherFrame<AnyMessage<'static>>>>,
    actions: Vec<Action<'static>>,
    cleanup_commmands: Vec<Command>,
    process: Vec<Option<tokio::process::Child>>,
    save: HashMap<String, String>,
}

impl Executor {
    pub async fn new(test: Test<'static>, test_name: String) -> Executor {
        let mut save: HashMap<String, String> = HashMap::new();
        let mut process: Vec<Option<tokio::process::Child>> = vec![];
        for command in test.setup_commmands {
            if command.command == "kill" {
                let index: usize = command.args[0].parse().unwrap();
                let p = process[index].as_mut();
                let mut pid = p.as_ref().unwrap().id();
                // Kill process
                p.unwrap().kill().await;
                // Wait until the process is killed to move on
                while let Some(i) = pid {
                    let p = process[index].as_mut();
                    pid = p.as_ref().unwrap().id();
                    p.unwrap().kill().await;
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }
                let p = process[index].as_mut();
            } else if command.command == "sleep" {
                let ms: u64 = command.args[0].parse().unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            } else {
                let p = os_command(
                    &command.command,
                    command.args.iter().map(String::as_str).collect(),
                    command.conditions,
                )
                .await;
                process.push(p);
            }
        }
        match (test.as_dowstream, test.as_upstream) {
            (Some(as_down), Some(as_up)) => {
                let (recv_from_down, send_to_down) = setup_as_upstream(
                    as_up.addr,
                    as_up.keys,
                    test.execution_commands,
                    &mut process,
                )
                .await;
                let (recv_from_up, send_to_up) =
                    setup_as_downstream(as_down.addr, as_down.key).await;
                Self {
                    name: Arc::new(test_name.clone()),
                    send_to_down: Some(send_to_down),
                    recv_from_down: Some(recv_from_down),
                    send_to_up: Some(send_to_up),
                    recv_from_up: Some(recv_from_up),
                    actions: test.actions,
                    cleanup_commmands: test.cleanup_commmands,
                    process,
                    save,
                }
            }
            (None, Some(as_up)) => {
                let (recv_from_down, send_to_down) = setup_as_upstream(
                    as_up.addr,
                    as_up.keys,
                    test.execution_commands,
                    &mut process,
                )
                .await;
                Self {
                    name: Arc::new(test_name.clone()),
                    send_to_down: Some(send_to_down),
                    recv_from_down: Some(recv_from_down),
                    send_to_up: None,
                    recv_from_up: None,
                    actions: test.actions,
                    cleanup_commmands: test.cleanup_commmands,
                    process,
                    save,
                }
            }
            (Some(as_down), None) => {
                let (recv_from_up, send_to_up) =
                    setup_as_downstream(as_down.addr, as_down.key).await;
                Self {
                    name: Arc::new(test_name.clone()),
                    send_to_down: None,
                    recv_from_down: None,
                    send_to_up: Some(send_to_up),
                    recv_from_up: Some(recv_from_up),
                    actions: test.actions,
                    cleanup_commmands: test.cleanup_commmands,
                    process,
                    save,
                }
            }
            (None, None) => Self {
                name: Arc::new(test_name.clone()),
                send_to_down: None,
                recv_from_down: None,
                send_to_up: None,
                recv_from_up: None,
                actions: test.actions,
                cleanup_commmands: test.cleanup_commmands,
                process,
                save,
            },
        }
    }

    pub async fn execute(mut self) {
        let mut success = true;
        for action in self.actions {
            if let Some(doc) = action.actiondoc {
                println!("actiondoc: {}", doc);
            }
            let (sender, recv) = match action.role {
                Role::Upstream => (
                    self.send_to_down
                        .as_ref()
                        .expect("Action require executor to act as upstream"),
                    self.recv_from_down
                        .as_ref()
                        .expect("Action require executor to act as upstream"),
                ),
                Role::Downstream => (
                    self.send_to_up
                        .as_ref()
                        .expect("Action require executor to act as downstream"),
                    self.recv_from_up
                        .as_ref()
                        .expect("Action require executor to act as downstream"),
                ),
                Role::Proxy => panic!("Action can be either executed as Downstream or Upstream"),
            };
            for message_ in action.messages {
                let tbd = message_.2.clone();
                let message = message_.1.clone();
                let frame = message_.0;
                //dbg!(message.1); // setup connection success
                //dbg!(message.2); // vettore vuoto, quello che c'e' dopo lo salta e va
                if tbd.len() == 0 {
                   println!("SEND {:#?}", message);
                   match sender.send(frame).await {
                       Ok(_) => (),
                       Err(_) => panic!(),
                   }
                } else {
                    let message_modified = change_fields(message, tbd, self.save.clone());
                    let modified_frame = StandardEitherFrame::Sv2(message_modified.clone().try_into().unwrap());
                    println!("SEND {:#?}", message_modified);
                    match sender.send(modified_frame).await {
                        Ok(_) => (),
                        Err(_) => panic!(),
                    }
                }
            }
            let mut rs = 0;
            for result in &action.result {
                rs += 1;
                println!("Working on result {}/{}: {}", rs, action.result.len(), result);

                // If the connection should drop at this point then let's just break the loop
                // Can't do anything else after the connection drops.
                if *result == ActionResult::CloseConnection {
                    recv.recv()
                        .await
                        .expect_err("Expecting the connection to be closed: wasn't");
                    break;
                }
                let message = match recv.recv().await {
                    Ok(message) => message,
                    Err(_) => {
                        success = false;
                        println!("Connection closed before receiving the message");
                        break;
                    },
                };
                let mut message: Sv2Frame<AnyMessage<'static>, _> = message.try_into().unwrap();
                println!("RECV {:#?}", message);
                let header = message.get_header().unwrap();
                let payload = message.payload();
                // next function print type of variables
                //print_type_of(&payload);
                let mut message_hashmap: HashMap<std::string::String, PoolMessages<'static>> = HashMap::new();
                match result {
                    ActionResult::MatchMessageType(message_type) => {
                        if header.msg_type() != *message_type {
                            println!(
                                "WRONG MESSAGE TYPE expected: {} received: {}",
                                message_type,
                                header.msg_type()
                            );
                            success = false;
                            break;
                        } else {
                            println!("MATCHED MESSAGE TYPE {}", message_type);
                        }
                    }
                    ActionResult::MatchMessageField((
                        subprotocol,
                        message_type,
                        field_data, // Vec<(String, Sv2Type)>
                    )) => {
                        if subprotocol.as_str() == "CommonMessage" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::CommonMessages::SetupConnection(m)) => {
                                    if message_type.as_str() == "SetupConnection" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::CommonMessages::SetupConnectionError(m)) => {
                                    if message_type.as_str() == "SetupConnectionError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::CommonMessages::SetupConnectionSuccess(m)) => {
                                    if message_type.as_str() == "SetupConnectionSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::CommonMessages::ChannelEndpointChanged(m)) => {
                                    if message_type.as_str() == "ChannelEndpointChanged" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Err(e) => panic!("{:?}", e),
                            }
                        } else if subprotocol.as_str() == "MiningProtocol" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::Mining::OpenExtendedMiningChannel(m)) => {
                                    print_type_of(&m);
                                    if message_type.as_str() == "OpenExtendedMiningChannel" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::OpenExtendedMiningChannelSuccess(m)) => {
                                    if message_type.as_str() == "OpenExtendedMiningChannelSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::OpenStandardMiningChannel(m)) => {
                                    if message_type.as_str() == "OpenStandardMiningChannel" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::OpenStandardMiningChannelSuccess(m)) => {
                                    if message_type.as_str() == "OpenStandardMiningChannelSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg,field_data);
                                    }
                                },
                                Ok(parsers::Mining::CloseChannel(m)) => {
                                    if message_type.as_str() == "CloseChannel" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::NewMiningJob(m)) => {
                                    if message_type.as_str() == "NewMiningJob" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::NewExtendedMiningJob(m)) => {
                                    if message_type.as_str() == "NewExtendedMiningJob" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetTarget(m)) => {
                                    if message_type.as_str() == "SetTarget" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SubmitSharesError(m)) => {
                                    if message_type.as_str() == "SubmitSharesError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SubmitSharesStandard(m)) => {
                                    if message_type.as_str() == "SubmitSharesStandard" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SubmitSharesSuccess(m)) => {
                                    if message_type.as_str() == "SubmitSharesSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SubmitSharesExtended(m)) => {
                                    if message_type.as_str() == "SubmitSharesExtended" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetCustomMiningJob(m)) => {
                                    if message_type.as_str() == "SetCustomMiningJob" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetCustomMiningJobError(m)) => {
                                    if message_type.as_str() == "SetCustomMiningJobError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::OpenMiningChannelError(m)) => {
                                    if message_type.as_str() == "OpenMiningChannelError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::Reconnect(m)) => {
                                    if message_type.as_str() == "Reconnect" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetCustomMiningJobSuccess(m)) => {
                                    if message_type.as_str() == "SetCustomMiningJobSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetExtranoncePrefix(m)) => {
                                    if message_type.as_str() == "SetExtranoncePrefix" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetGroupChannel(m)) => {
                                    if message_type.as_str() == "SetGroupChannel" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::SetNewPrevHash(m)) => {
                                    if message_type.as_str() == "SetNewPrevHash" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::UpdateChannel(m)) => {
                                    if message_type.as_str() == "UpdateChannel" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::Mining::UpdateChannelError(m)) => {
                                    if message_type.as_str() == "UpdateChannelError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else if subprotocol.as_str() == "JobNegotiationProtocol" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::JobNegotiation::AllocateMiningJobTokenSuccess(m)) => {
                                    if message_type.as_str() == "AllocateMiningJobTokenSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                }
                                Ok(parsers::JobNegotiation::AllocateMiningJobToken(m)) => {
                                    if message_type.as_str() == "AllocateMiningJobToken" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                }
                                Ok(parsers::JobNegotiation::CommitMiningJob(m)) => {
                                    if message_type.as_str() == "CommitMiningJob" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                }
                                Ok(parsers::JobNegotiation::CommitMiningJobSuccess(m)) => {
                                    if message_type.as_str() == "CommitMiningJobSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                }
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else if subprotocol.as_str() == "TemplateDistributionProtocol" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::TemplateDistribution::SubmitSolution(m)) => {
                                    if message_type.as_str() == "SubmitSolution" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::NewTemplate(m)) => {
                                    if message_type.as_str() == "NewTemplate" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::SetNewPrevHash(m)) => {
                                    if message_type.as_str() == "SetNewPrevHash" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::CoinbaseOutputDataSize(m)) => {
                                    if message_type.as_str() == "CoinbaseOutputDataSize" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionData(m)) => {
                                    if message_type.as_str() == "RequestTransactionData" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionDataError(m)) => {
                                    if message_type.as_str() == "RequestTransactionDataError" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionDataSuccess(m)) => {
                                    if message_type.as_str() == "RequestTransactionDataSuccess" {
                                        let msg = serde_json::to_value(&m).unwrap();
                                        check_each_field(msg, field_data);
                                    }
                                },
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else {
                            println!(
                                "match_message_field subprotocol not valid - received: {}",
                                subprotocol
                            );
                            panic!()
                        }
                    }
                    ActionResult::GetMessageField{
                        subprotocol, 
                        message_type, 
                        fields
                        } => {
                        if subprotocol.as_str() == "CommonMessage" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::CommonMessages::SetupConnection(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::CommonMessages::SetupConnectionError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::CommonMessages::ChannelEndpointChanged(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::CommonMessages::SetupConnectionSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else if subprotocol.as_str() == "MiningProtocol" {
                            match (header.msg_type(), payload).try_into() {

                                Ok(parsers::Mining::OpenExtendedMiningChannel(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::OpenExtendedMiningChannelSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::OpenStandardMiningChannel(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::OpenStandardMiningChannelSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::CloseChannel(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::NewMiningJob(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::NewExtendedMiningJob(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetTarget(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SubmitSharesError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SubmitSharesStandard(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SubmitSharesSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SubmitSharesExtended(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::OpenMiningChannelError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::Reconnect(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetCustomMiningJobSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetExtranoncePrefix(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetGroupChannel(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetNewPrevHash(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::UpdateChannel(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::UpdateChannelError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetCustomMiningJob(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetCustomMiningJobSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::Mining::SetCustomMiningJobError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else if subprotocol.as_str() == "JobNegotiationProtocol" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::JobNegotiation::AllocateMiningJobTokenSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                }
                                Ok(parsers::JobNegotiation::AllocateMiningJobToken(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                }
                                Ok(parsers::JobNegotiation::CommitMiningJob(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                }
                                Ok(parsers::JobNegotiation::CommitMiningJobSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                }
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else if subprotocol.as_str() == "TemplateDistributionProtocol" {
                            match (header.msg_type(), payload).try_into() {
                                Ok(parsers::TemplateDistribution::SubmitSolution(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::NewTemplate(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::SetNewPrevHash(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::CoinbaseOutputDataSize(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionData(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionDataError(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Ok(parsers::TemplateDistribution::RequestTransactionDataSuccess(m)) => {
                                    let mess = serde_json::to_value(&m).unwrap();
                                    self.save = save_message_field(mess, self.save.clone(), fields);
                                },
                                Err(e) => panic!("err {:?}", e),
                            }
                        } else {
                            println!(
                                "GetMessageField not implemented for this protocol",
                            );
                            panic!()
                        };
                    }
                    ActionResult::MatchMessageLen(message_len) => {
                        if payload.len() != *message_len {
                            println!(
                                "WRONG MESSAGE len expected: {} received: {}",
                                message_len,
                                payload.len()
                            );
                            success = false;
                            break;
                        }
                    }
                    ActionResult::MatchExtensionType(ext_type) => {
                        if header.ext_type() != *ext_type {
                            println!(
                                "WRONG EXTENSION TYPE expected: {} received: {}",
                                ext_type,
                                header.ext_type()
                            );
                            success = false;
                            break;
                        }
                    }
                    ActionResult::CloseConnection => {
                        todo!()
                    }
                    ActionResult::None => todo!(),
                }
            }
        }
        for command in self.cleanup_commmands {
            os_command(
                &command.command,
                command.args.iter().map(String::as_str).collect(),
                command.conditions,
            )
            // Give time to the last cleanup command to return before exit from the process
            .await
            .unwrap()
            .wait()
            .await
            .unwrap();
        }
        let mut child_no = 0;

        for child in self.process {
            if let Some(mut child) = child {
                // Spawn a task to read the child process's stdout and write it to the file
                let stdout = child.stdout.take().unwrap();
                let mut stdout_reader = BufReader::new(stdout);
                child_no = child_no + 1;
                let test_name = self.name.clone();
                tokio::spawn(async move {
                    let test_name = &*test_name;
                    let mut file = File::create(format!("{}.child-{}.log", test_name, child_no)).await.unwrap();
                    let mut stdout_writer = BufWriter::new(&mut file);

                    copy(&mut stdout_reader, &mut stdout_writer).await.unwrap();
                });


                while let Some(i) = &child.id() {
                    // Sends kill signal and waits 1 second before checking to ensure child was killed
                    child.kill().await;
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }
            }
        }
        if !success {
            panic!("test failed!!!");
        }
    }
}
fn change_fields<'a>(m: AnyMessage<'a>, tbd: Vec<(String,String)>, values: HashMap<String, String>) -> AnyMessage<'static> {
    let mut tbd = tbd.clone();
    let next = tbd.pop().expect("tbd cannot be empty");
    let keyword = next.1;
    let field_name = next.0;
    let value = values.get(&keyword).expect("value not found for the keyword");

   match m.clone() {
       AnyMessage::Common(m) => {
           let path = match m {
               CommonMessages::ChannelEndpointChanged(_) => "ChannelEndpointChanged",
               CommonMessages::SetupConnection(_) => "SetupConnection",
               CommonMessages::SetupConnectionError(_) => "SetupConnectionError",
               CommonMessages::SetupConnectionSuccess(_) => "SetupConnectionSuccess",
           };
           let mut message_as_serde_value = serde_json::to_value(&m).unwrap();
           let m_ = change_value_of_serde_field(message_as_serde_value, path, value, field_name);
           if tbd.len() == 0 {
               return m_;
           } else {
               return change_fields(m_, tbd, values);
           }
       },
       AnyMessage::Mining(m) => {
           let path = match m {
               parsers::Mining::CloseChannel(_) => "CloseChannel",
               parsers::Mining::NewExtendedMiningJob(_) => "NewExtendedMiningJob",
               parsers::Mining::NewMiningJob(_) => "NewMiningJob",
               parsers::Mining::OpenExtendedMiningChannel(_) => "OpenExtendedMiningChannel",
               parsers::Mining::OpenExtendedMiningChannelSuccess(_) => "OpenExtendedMiningChannelSuccess",
               parsers::Mining::OpenMiningChannelError(_) => "OpenMiningChannelError",
               parsers::Mining::OpenStandardMiningChannel(_) => "OpenStandardMiningChannel",
               parsers::Mining::OpenStandardMiningChannelSuccess(_) => "",
               parsers::Mining::Reconnect(_) => "Reconnect",
               parsers::Mining::SetCustomMiningJob(_) => "SetCustomMiningJob",
               parsers::Mining::SetCustomMiningJobError(_) => "",
               parsers::Mining::SetCustomMiningJobSuccess(_) => "",
               parsers::Mining::SetExtranoncePrefix(_) => "SetExtranoncePrefix",
               parsers::Mining::SetGroupChannel(_) => "SetGroupChannel",
               parsers::Mining::SetNewPrevHash(_) => "SetNewPrevHash",
               parsers::Mining::SetTarget(_) => "SetTarget",
               parsers::Mining::SubmitSharesError(_) => "SubmitSharesError",
               parsers::Mining::SubmitSharesExtended(_) => "SubmitSharesExtended",
               parsers::Mining::SubmitSharesStandard(_) => "SubmitSharesStandard",
               parsers::Mining::SubmitSharesSuccess(_) => "SubmitSharesSuccess",
               parsers::Mining::UpdateChannel(_) => "UpdateChannel",
               parsers::Mining::UpdateChannelError(_) => "UpdateChannelError",
           };
           let mut message_as_serde_value = serde_json::to_value(&m).unwrap();
           match message_as_serde_value.pointer_mut(&format!("/{}/{}", path, field_name.as_str())) {
             Some(field_value) => {
                 let value = value.parse::<i32>().unwrap();
                 *field_value = json!(value);
             }
             _ => panic!("value not found")
           }
           let message_as_string = serde_json::to_string(&message_as_serde_value).unwrap();
           let m_ = into_static(AnyMessage::Mining(serde_json::from_str(&message_as_string).unwrap()));
           if tbd.len() == 0 {
               return m_;
           } else {
               return change_fields(m_, tbd, values);
           }
       },
       AnyMessage::JobNegotiation(m) => {
            let path = match m {
            roles_logic_sv2::parsers::JobNegotiation::AllocateMiningJobToken(_) => "AllocateMiningJobToken",
            roles_logic_sv2::parsers::JobNegotiation::AllocateMiningJobTokenSuccess(_) => "AllocateMiningJobTokenSuccess",
            roles_logic_sv2::parsers::JobNegotiation::CommitMiningJob(_) => "CommitMiningJob",
            roles_logic_sv2::parsers::JobNegotiation::CommitMiningJobSuccess(_) => "CommitMiningJobSuccess",
        };
           let mut message_as_serde_value = serde_json::to_value(&m).unwrap();
           let m_ = change_value_of_serde_field(message_as_serde_value, path, value, field_name);
           if tbd.len() == 0 {
               return m_;
           } else {
               return change_fields(m_, tbd, values);
           }
        },
        AnyMessage::TemplateDistribution(m) => {
            let path = match m {
                roles_logic_sv2::parsers::TemplateDistribution::CoinbaseOutputDataSize(_) => "CoinbaseOutputDataSize",
                roles_logic_sv2::parsers::TemplateDistribution::NewTemplate(_) => "NewTemplate",
                roles_logic_sv2::parsers::TemplateDistribution::RequestTransactionData(_) => "RequestTransactionData",
                roles_logic_sv2::parsers::TemplateDistribution::RequestTransactionDataError(_) => "RequestTransactionDataError",
                roles_logic_sv2::parsers::TemplateDistribution::RequestTransactionDataSuccess(_) => "RequestTransactionDataSuccess",
                roles_logic_sv2::parsers::TemplateDistribution::SetNewPrevHash(_) => "SetNewPrevHash",
                roles_logic_sv2::parsers::TemplateDistribution::SubmitSolution(_) => "SubmitSolution",
            };
            let mut message_as_serde_value = serde_json::to_value(&m).unwrap();
            let m_ = change_value_of_serde_field(message_as_serde_value, path, value, field_name);
            if tbd.len() == 0 {
                return m_;
            } else {
                return change_fields(m_, tbd, values);
            }
        },
   }
}

fn change_value_of_serde_field (mut message_as_serde_value: serde_json::Value, path: &str, value: &str, field_name: String) -> AnyMessage<'static> {
    match message_as_serde_value.pointer_mut(&format!("/{}/{}", path, field_name.as_str())) {
      Some(field_value) => {
          let value = value.parse::<i32>().unwrap();
          *field_value = json!(value);
      }
      _ => panic!("value not found")
    }
    let message_as_string = serde_json::to_string(&message_as_serde_value).unwrap();
    into_static(AnyMessage::Mining(serde_json::from_str(&message_as_string).unwrap()))
}

fn save_message_field (mess: serde_json::Value, mut save: HashMap<String, String>, fields: &Vec<(String, String)>) -> HashMap<String, String> {
    for field in fields {
        let key = field.1.clone();
        let field_name = &field.0;
        let to_save = message_to_value(&mess, field_name);
        save.insert(key,to_save);
    };
    save
}


fn check_msg_field(msg: serde_json::Value, field_name: &str, value_type: &str, field: &Sv2Type) {
    let msg = msg.as_object().unwrap();
    let value = msg
        .get(field_name)
        .expect("match_message_field field name is not valid")
        .clone();
    let value = serde_json::to_string(&value).unwrap();
    let value = format!(r#"{{"{}":{}}}"#, value_type, value);
    let value: crate::Sv2Type = serde_json::from_str(&value).unwrap();
    assert!(
        field == &value,
        "match_message_field value is incorrect. Expected = {:?}, Recieved = {:?}",
        field,
        value
    )
}

fn check_each_field(msg: serde_json::Value, field_info: &Vec<(String, Sv2Type)>) {
    for field in field_info {
        let value_type = serde_json::to_value(&field.1)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .to_string();

        check_msg_field(msg.clone(), &field.0, &value_type, &field.1)
    }
}
fn message_to_value(m: & serde_json::Value,field: &str) -> String {
    let msg = m.as_object().unwrap();
    let value = msg.get(field).unwrap();
    value.to_string()
}
#[test]

fn test_1() {
    let message_as_string = r#"{"OpenExtendedMiningChannelSuccess":{"channel_id":1,"extranonce_prefix":[[16],[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]],"extranonce_size":16,"request_id":6,"target":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,255,255,255,255,255,255,255,255]}}"#;
    let message_: roles_logic_sv2::parsers::Mining<'_> = serde_json::from_str(&message_as_string).unwrap();
}
