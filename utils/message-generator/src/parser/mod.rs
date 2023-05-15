mod actions;
mod frames;
mod sv2_messages;

use crate::{Action, Command, Test};
use codec_sv2::{buffer_sv2::Slice, Frame, Sv2Frame};
use frames::Frames;
use roles_logic_sv2::parsers::AnyMessage;
use serde_json::{Map, Value};
use std::{collections::HashMap, convert::TryInto};
use sv2_messages::TestMessageParser;

/// invece di rbd usa replace_field
/// mettere il tbd dentro i messages1
#[derive(Debug)]
pub enum Parser<'a> {
    /// Parses any number or combination of messages to be later used by an action identified by
    /// message id.
    /// TODO scrivere a cosa corrispodono i campi di Vec<(String, Strin)>
    Step1(HashMap<String, (AnyMessage<'a>, Vec<(String, String)>)>),
    /// Serializes messages into `Sv2Frames` identified by message id.
    Step2 {
        /// il secondo Vec<(String,String)> corrisponde al tbd 
        messages: HashMap<String, (AnyMessage<'a>,Vec<(String,String)>)>,
        frames: HashMap<String, Sv2Frame<AnyMessage<'a>, Slice>>,
    },
    /// Parses the setup, execution, and cleanup shell commands, roles, and actions.
    Step3 {
        messages: HashMap<String, (AnyMessage<'a>, Vec<(String,String)>)>,
        frames: HashMap<String, Sv2Frame<AnyMessage<'a>, Slice>>,
        actions: Vec<Action<'a>>,
    },
    /// Prepare for execution.
    Step4(Test<'a>),
}

impl<'a> Parser<'a> {
    pub fn parse_test<'b: 'a>(test: &'b str) -> Test<'a> {
        let step1 = Self::initialize(test);
        let step2 = step1.next_step(test);
        let step3 = step2.next_step(test);
        let step4 = step3.next_step(test);
        match step4 {
            Self::Step4(test) => test,
            _ => unreachable!(),
        }
    }

    fn initialize<'b: 'a>(test: &'b str) -> Self {
        let messages = TestMessageParser::from_str(test);
        ///let mut tbd = Vec::new();
        /// non si puo' iterare su uno struct
        ///for message in messages.iter() {
        ///    if let Some(tbd_) = message.get("tbd") {
        ///       tbd = tbd.push((tbd_[0].to_string(),tbd[1].to_string()));
        ///       ///rimuovere il tbd da message
        ///    }
        ///}
        //let step1 = Self::Step1((messages, tbd).into_map());
        let step1 = Self::Step1(messages.into_map());
        step1
    }

    fn next_step<'b: 'a>(self, test: &'b str) -> Self {
        match self {
            Self::Step1(messages) => {
                //println!("STEP 1");
                //dbg!(messages.clone());
                let (frames, messages) = Frames::from_step_1(test, messages.clone());
                //let mut tbd: Vec<(String, String)> = Vec::new();
                //for (key, value) in messages {
                //    tbd.append(&mut value.1);
                //}
                // messages deve essere HashMap<String, AnyMessage<'a>> quindi bisogna rifare
                // l'hashmap togliendo Vec<(String,String)> e mettendolo in un tbd
                Self::Step2 {
                    messages,
                    frames: frames.frames,
                }
            }
            Self::Step2 { messages, frames} => {
                //println!("STEP 2");
                //dbg!(messages.clone());
                let actions = actions::ActionParser::from_step_2(test, frames.clone(), messages.clone());
                Self::Step3 {
                    messages,
                    frames,
                    actions,
                }
            }
            Self::Step3 {
                messages: _,
                frames: _,
                actions,
            } => {
                let test: Map<String, Value> = serde_json::from_str(test).unwrap();
                let setup_commands = test.get("setup_commands").unwrap().as_array().unwrap();
                let execution_commands =
                    test.get("execution_commands").unwrap().as_array().unwrap();
                let cleanup_commands = test.get("cleanup_commands").unwrap().as_array().unwrap();

                let setup_commmands: Vec<Command> = setup_commands
                    .iter()
                    .map(|s| serde_json::from_value(s.clone()).unwrap())
                    .collect();
                let execution_commands: Vec<Command> = execution_commands
                    .iter()
                    .map(|s| serde_json::from_value(s.clone()).unwrap())
                    .collect();
                let cleanup_commmands: Vec<Command> = cleanup_commands
                    .iter()
                    .map(|s| serde_json::from_value(s.clone()).unwrap())
                    .collect();

                let (as_upstream, as_dowstream) = match test.get("role").unwrap().as_str().unwrap()
                {
                    "client" => {
                        let downstream = test.get("downstream").unwrap();
                        let ip = downstream.get("ip").unwrap().as_str().unwrap();
                        let port = downstream.get("port").unwrap().as_u64().unwrap() as u16;
                        let pub_key = downstream
                            .get("pub_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        (
                            None,
                            Some(crate::Downstream {
                                addr: std::net::SocketAddr::new(ip.parse().unwrap(), port),
                                key: pub_key.map(|k| k.to_string().try_into().unwrap()),
                            }),
                        )
                    }
                    "server" => {
                        let upstream = test.get("upstream").unwrap();
                        let ip = upstream.get("ip").unwrap().as_str().unwrap();
                        let port = upstream.get("port").unwrap().as_u64().unwrap() as u16;
                        let pub_key = upstream
                            .get("pub_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        let secret_key = upstream
                            .get("secret_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        let keys = match (pub_key, secret_key) {
                            (Some(p), Some(s)) => Some((
                                p.to_string().try_into().unwrap(),
                                s.to_string().try_into().unwrap(),
                            )),
                            (None, None) => None,
                            _ => panic!(),
                        };
                        (
                            Some(crate::Upstream {
                                addr: std::net::SocketAddr::new(ip.parse().unwrap(), port),
                                keys,
                            }),
                            None,
                        )
                    }
                    "proxy" => {
                        let downstream = test.get("downstream").unwrap();
                        let ip = downstream.get("ip").unwrap().as_str().unwrap();
                        let port = downstream.get("port").unwrap().as_u64().unwrap() as u16;
                        let pub_key = downstream
                            .get("pub_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        let downstream = crate::Downstream {
                            addr: std::net::SocketAddr::new(ip.parse().unwrap(), port),
                            key: pub_key.map(|k| k.to_string().try_into().unwrap()),
                        };

                        let upstream = test.get("upstream").unwrap();
                        let ip = upstream.get("ip").unwrap().as_str().unwrap();
                        let port = upstream.get("port").unwrap().as_u64().unwrap() as u16;
                        let pub_key = upstream
                            .get("pub_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        let secret_key = upstream
                            .get("secret_key")
                            .map(|a| a.as_str().unwrap().to_string());
                        let keys = match (pub_key, secret_key) {
                            (Some(p), Some(s)) => Some((
                                p.to_string().try_into().unwrap(),
                                s.to_string().try_into().unwrap(),
                            )),
                            (None, None) => None,
                            _ => panic!(),
                        };
                        let upstream = crate::Upstream {
                            addr: std::net::SocketAddr::new(ip.parse().unwrap(), port),
                            keys,
                        };
                        (Some(upstream), Some(downstream))
                    }
                    "none" => (None, None),
                    role @ _ => panic!("Unknown role: {}", role),
                };

                let test = Test {
                    actions,
                    as_upstream,
                    as_dowstream,
                    setup_commmands,
                    execution_commands,
                    cleanup_commmands,
                };
                Self::Step4(test)
            }
            Parser::Step4(test) => Parser::Step4(test),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use binary_sv2::*;

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct TestStruct<'decoder> {
        #[serde(borrow)]
        test_b016m: B016M<'decoder>,
        #[serde(borrow)]
        test_b0255: B0255<'decoder>,
        #[serde(borrow)]
        test_b032: B032<'decoder>,
        #[serde(borrow)]
        test_b064: B064K<'decoder>,
        #[serde(borrow)]
        test_seq_064k_bool: Seq064K<'decoder, bool>,
        #[serde(borrow)]
        test_seq_064k_b064k: Seq064K<'decoder, B064K<'decoder>>,
    }

    #[test]
    fn it_parse_test() {
        let test = std::fs::read_to_string("./test.json").unwrap();
        let step1 = Parser::initialize(&test);

        let step2 = step1.next_step(&test);
        let step3 = step2.next_step(&test);
        let step4 = step3.next_step(&test);
        match step4 {
            Parser::Step4(test) => {
                assert!(test.actions.len() == 2);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn it_parse_sequences() {
        let test_json = r#"
        {
            "test_b016m": [1,1],
            "test_b0255": [1,1],
            "test_b032": [1,1],
            "test_b064": [1,1],
            "test_seq_064k_bool": [true,false],
            "test_seq_064k_b064k": [[1,2],[3,4]]
        }
        "#;
        let test_struct: TestStruct = serde_json::from_str(test_json).unwrap();
        assert!(test_struct.test_b016m == vec![1, 1].try_into().unwrap());
        assert!(test_struct.test_b0255 == vec![1, 1].try_into().unwrap());
        assert!(test_struct.test_b032 == vec![1, 1].try_into().unwrap());
        assert!(test_struct.test_b064 == vec![1, 1].try_into().unwrap());
        assert!(test_struct.test_b064 == vec![1, 1].try_into().unwrap());
        assert!(test_struct.test_seq_064k_bool.into_inner() == vec![true, false]);
        assert!(
            test_struct.test_seq_064k_b064k.into_inner()
                == vec![
                    vec![1, 2].try_into().unwrap(),
                    vec![3, 4].try_into().unwrap(),
                ]
        );
    }
}
