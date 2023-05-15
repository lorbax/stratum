use crate::{Action, ActionResult, Role, Sv2Type};
use codec_sv2::{buffer_sv2::Slice, StandardEitherFrame, Sv2Frame};
use roles_logic_sv2::parsers::AnyMessage;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::any::Any;
use std::any::TypeId;

pub struct ActionParser {}

impl ActionParser {
    pub fn from_step_2<'a, 'b: 'a>(
        test: &'b str,
        frames: HashMap<String, Sv2Frame<AnyMessage<'a>, Slice>>,
        messages: HashMap<String, (AnyMessage<'a>,Vec<(String,String)>)>
    ) -> Vec<Action<'a>> {
        let test: Map<String, Value> = serde_json::from_str(test).unwrap();
        let actions = test.get("actions").unwrap().as_array().unwrap();
        let mut result = vec![];
        let tbd_action: Vec<(String, String)> = Vec::new();
        for action in actions {
            let role = match action.get("role").unwrap().as_str().unwrap() {
                "client" => Role::Downstream,
                "server" => Role::Upstream,
                role @ _ => panic!("Unknown role: {}", role),
            };
            let mut action_frames = vec![];
            let ids = action.get("message_ids").unwrap().as_array().unwrap();
            for id in ids {
                let frame = frames
                    .get(id.as_str().unwrap())
                    .unwrap_or_else(|| {
                        panic!("Frame id not found: {} Impossible to parse action", id)
                    })
                    .clone();
                let frame = StandardEitherFrame::Sv2(frame);
                let message = messages 
                .get(id.as_str().unwrap())
                .unwrap_or_else(|| {
                    panic!("Message id not found: {} Impossible to parse action", id)
                })
                .clone();
                action_frames.push((frame, message.0, message.1));
            }

            let actiondoc = match action.get("actiondoc") {
                Some(T) => Some(T.to_string()),
                None => None,
            };
            let mut action_results = vec![];
            let results = action.get("results").unwrap().as_array().unwrap();
            for result in results {
                match result.get("type").unwrap().as_str().unwrap() {
                    "match_message_type" => {
                        let message_type = u8::from_str_radix(&result.get("value").unwrap().as_str().unwrap()[2..], 16).expect("Result message_type should be an hex value starting with 0x and not bigger than 0xff");
                        action_results.push(ActionResult::MatchMessageType(message_type));
                    }
                    // inserire get_message_field
                    "get_message_field" => {
                        //let mut sv2_type = result.get("value").unwrap().clone();
                        //let sv2_type_ = sv2_type.as_array();
                        //for item in sv2_type_ {
                        //    if let Some(tbds) = item.get("get_field").unwrap().as_array() {
                        //        for tbd in tbds {
                        //            tbd_action + &tbd;
                        //        }
                        //    }
                        //}
                        //if let Some(map) = sv2_type.as_object_mut() {
                        //    map.remove("get_field");
                        //} 
                        let sv2_type = result.get("value").unwrap().clone();
                        let sv2_type: (String, String, Vec<(String, String)>) =
                            serde_json::from_value(sv2_type)
                                .expect("match_message_field values not correct");
                        let get_message_field = ActionResult::GetMessageField{
                            subprotocol: sv2_type.0, 
                            message_type: sv2_type.1, 
                            fields: sv2_type.2
                        } ;
                        action_results.push(get_message_field);
                    }
                    "match_message_field" => {
                        let sv2_type = result.get("value").unwrap().clone();
                        let sv2_type: (String, String, Vec<(String, Sv2Type)>) =
                            serde_json::from_value(sv2_type)
                                .expect("match_message_field values not correct");
                        action_results.push(ActionResult::MatchMessageField(sv2_type));
                    }
                    "match_message_len" => {
                        let message_len = result.get("value").unwrap().as_u64().unwrap() as usize;
                        action_results.push(ActionResult::MatchMessageLen(message_len));
                    }
                    "match_extension_type" => {
                        let extension_type = result
                            .get("extension_type")
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .replace("_", "")
                            .parse::<u16>()
                            .unwrap();
                        action_results.push(ActionResult::MatchExtensionType(extension_type));
                    }
                    "close_connection" => {
                        action_results.push(ActionResult::CloseConnection);
                    }
                    "none" => {
                        action_results.push(ActionResult::None);
                    }
                    type_ @ _ => panic!("Unknown result type {}", type_),
                }
            }

            let action = Action {
                messages: action_frames,
                //tbd_action,
                result: action_results,
                role,
                actiondoc,
            };
            result.push(action);
        }
        result
    }
}
