use super::super::indexes::{WatchDiff};
use super::super::ops::{Internable, Interner, RawChange, RunLoopMessage};
use std::sync::mpsc::{Sender};
use super::Watcher;

extern crate serde_json;
extern crate serde;
use self::serde_json::{Map, Value, Number};

pub struct JsonWatcher {
    name: String,
    outgoing: Sender<RunLoopMessage>,
}

impl JsonWatcher {
    pub fn new(outgoing: Sender<RunLoopMessage>) -> JsonWatcher {
        JsonWatcher { name: "json".to_string(), outgoing }
    }
}

impl Watcher for JsonWatcher {
    fn get_name(& self) -> String {
        self.name.clone()
    }
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    fn on_diff(&mut self, interner:&mut Interner, diff:WatchDiff) {
        let mut record_map = Map::new();
        let mut changes: Vec<RawChange> = vec![];
        let mut id = "".to_string();
        for add in diff.adds {
            let kind = Internable::to_string(interner.get_value(add[0]));
            let record_id = Internable::to_string(interner.get_value(add[1]));
            
            match kind.as_ref() {
                "encode/target" => {
                    id = Internable::to_string(interner.get_value(add[2]));
                },
                "encode/eav" => {
                    let e = Internable::to_string(interner.get_value(add[2]));
                    let a = Internable::to_string(interner.get_value(add[3]));
                    let v: Value = match interner.get_value(add[4]) {
                        internable@&Internable::Number(_) => Value::Number(Number::from_f64(Internable::to_number(internable) as f64).unwrap()),
                        &Internable::String(ref n) => Value::String(String::from(n.clone())),
                        _ => Value::Null
                    };
                    println!("Encoding: {:?} {:?} {:?}",e,a,v);
                    if record_map.contains_key(&e) {
                        let record = record_map.get_mut(&e).unwrap();
                        let sub_record = record.as_object_mut().unwrap();
                        sub_record.insert(a, v);
                        println!("Sub Record: {:?}",sub_record);
                    } else {
                        let mut new_record = Map::new();
                        new_record.insert(a, v);
                        println!("New Record: {:?}",new_record);
                        record_map.insert(e, Value::Object(new_record));
                    }
                },
                "decode" => {
                    println!("DECODING");
                    let value = Internable::to_string(interner.get_value(add[2]));
                    let v: Value = serde_json::from_str(&value).unwrap();
                    value_to_changes(&record_id.to_string(), "json-object", v, "json/decode", &mut changes);
                },
                _ => {},
            }
        }
        println!("Record Map: {:?}",record_map);
        if let Some(target_record) = record_map.get(&id) {
            let inner_map = target_record.as_object().unwrap();
            println!("Inner Map: {:?}", inner_map);
            let dereferenced_target = dereference(inner_map, &record_map);
            let json = serde_json::to_string(&dereferenced_target).unwrap();
            let change_id = format!("json/encode/change|{:?}",id);
            changes.push(new_change(&change_id, "tag", Internable::from_str("json/encode/change"), "json/encode"));
            changes.push(new_change(&change_id, "json-string", Internable::String(json), "json/encode"));
            changes.push(new_change(&change_id, "record", Internable::String(id), "json/encode"));
        }
        match self.outgoing.send(RunLoopMessage::Transaction(changes)) {
            Err(_) => (),
            _ => (),
        }   
    }
}

// Resolves all the object links in the flat map
fn dereference(target: &Map<String,Value>, flatmap: &Map<String,Value>) -> Map<String,Value> {
    let mut dereferenced = Map::new();
    for key in target.keys() {
        let value = target.get(key).unwrap();
        println!("DeRefVal: {:?}",value.as_str());
        match value {
            &Value::String(ref s) => println!("{:?}",s),
            &Value::Number(ref n) => println!("{:?}",n),
            x => println!("{:?}",x),
        }



        match value.as_str() {
            Some(v) => {
                if flatmap.contains_key(v) {
                    let value = flatmap.get(v).unwrap().as_object().unwrap();
                    dereferenced.insert(key.to_string(),Value::Object(dereference(value, flatmap)));
                } else {
                    dereferenced.insert(key.to_string(),value.clone());
                }
            },
            None => (),
        };
    }
    dereferenced
}
    
pub fn new_change(e: &str, a: &str, v: Internable, n: &str) -> RawChange {
    RawChange {e: Internable::from_str(e), a: Internable::from_str(a), v: v.clone(), n: Internable::from_str(n), count: 1}
}

pub fn value_to_changes(id: &str, attribute: &str, value: Value, node: &str, changes: &mut Vec<RawChange>) {
    match value {
        Value::Number(n) => {    
            if n.is_u64() { 
                let v = Internable::from_number(n.as_u64().unwrap() as f32); 
                changes.push(new_change(id,attribute,v,node));
            } else if n.is_i64() {
                let v = Internable::from_number(n.as_i64().unwrap() as f32); 
                changes.push(new_change(id,attribute,v,node));
            } else if n.is_f64() { 
                let v = Internable::from_number(n.as_f64().unwrap() as f32); 
                changes.push(new_change(id,attribute,v,node));
            };
        },
        Value::String(ref n) => {
            changes.push(new_change(id,attribute,Internable::String(n.clone()),node));
        },
        Value::Bool(ref n) => {
            let b = match n {
                &true => "true",
                &false => "false",
            };
            changes.push(new_change(id,attribute,Internable::from_str(b),node));
        },
        Value::Array(ref n) => {
            for (ix, value) in n.iter().enumerate() {
                let ix = ix + 1;
                let array_id = format!("array|{:?}|{:?}|{:?}", id, ix, value);
                let array_id = &array_id[..];
                changes.push(new_change(id,attribute,Internable::from_str(array_id),node));
                changes.push(new_change(array_id,"tag",Internable::from_str("array"),node));
                changes.push(new_change(array_id,"index",Internable::String(ix.to_string()),node));
                value_to_changes(array_id, "value", value.clone(), node, changes);
            }
        },
        Value::Object(ref n) => {
            let object_id = format!("{:?}",n);
            changes.push(new_change(id,attribute,Internable::String(object_id.clone()),node));
            changes.push(new_change(id,"tag",Internable::from_str("json-object"),node));
            for key in n.keys() {
                value_to_changes(&mut object_id.clone(), key, n[key].clone(), node, changes);
            }
        },
    _ => {},
    }  
}   

/*
#[derive(Debug)]
pub enum ChangeVec {
    Changes(Vec<RawChange>)
}

impl ChangeVec {
    pub fn new() -> ChangeVec {
        ChangeVec::Changes(Vec::new())
    }
}

impl<'de> Deserialize<'de> for ChangeVec {

    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        struct ChangeVecVisitor {
            marker: PhantomData<ChangeVec>
        }

        impl ChangeVecVisitor {
            fn new() -> Self {
                ChangeVecVisitor {
                    marker: PhantomData
                }
            }
        }

        impl<'de> Visitor<'de> for ChangeVecVisitor {
            type Value = ChangeVec;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("expecting a thing")
            }

            fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
                where M: MapAccess<'de>
            {
                let mut vec = Vec::new();
                while let Some(kv) = try!(access.next_entry()) {
                    vec.push(kv);
                }
                Ok(ChangeVec::new())
            }
        }

        deserializer.deserialize_any(ChangeVecVisitor::new())
    }
}*/