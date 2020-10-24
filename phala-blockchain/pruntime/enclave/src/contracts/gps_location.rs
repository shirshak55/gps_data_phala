use crate::std::collections::BTreeMap;


use serde::{Serialize,Deserialize};

use crate::contracts;
use crate::types::TxRef;
use crate::TransactionStatus;
use crate::contracts::AccountIdWrapper;
use crate::std::string::String;
use crate::std::vec::Vec;


// Logitude And Latitude
type LongLati = (f64,f64);


#[derive(Serialize, Deserialize, Debug, Default)]
pub struct GpsLocation {
    data: BTreeMap<AccountIdWrapper, LongLati>
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    SetLocation {
        latitude: f64,
        longitude: f64,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Error {
    InvalidRequest,
    NotAuthorized,
    NoData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
    GetLocation{
        account: AccountIdWrapper
    },
}


#[derive(Serialize, Deserialize, Debug)]
pub enum Response{
    Location{
        longitude: f64,
        latitude: f64
    },
    Error(Error)
}


impl GpsLocation {
    pub fn new() -> Self{
       Self{ data: BTreeMap::new() }
    }
}


impl contracts::Contract<Command, Request, Response> for GpsLocation {
    fn id(&self) -> contracts::ContractId { contracts::GPS_APP }


    fn handle_command(&mut self, account: &chain::AccountId, _txref: &TxRef, cmd: Command) -> TransactionStatus {
        println!("Inside Handle Command");
        println!("Contract 6 details {:?} {:?}",account,cmd);
        match cmd {
            Command::SetLocation {latitude, longitude} => {
                let account = AccountIdWrapper(account.clone());
                
                self.data.insert(account, (latitude, longitude));
                TransactionStatus::Ok
            }
        }
    }

    fn handle_query(&mut self,account: Option<&chain::AccountId>, req: Request) -> Response{
        println!("Inside Handle Query");
      
        if let Request::GetLocation{account}  = req {
            println!("Contract 6 Handle Query :::  \n {:?} \n {:?} \n {:?}",
                account.clone().to_string(),
                self.data.keys().map(|v| v.to_string()).collect::<Vec<String>>(),
                self.data.values().map(|v| v.0).collect::<Vec<f64>>(),
            );
            
            let hh = self.data.get(&account);

            println!("Find it:::{:?}", hh);

            if let Some(data) = hh {
                println!("Found the item returning the fucking location");
                return Response::Location{
                    latitude: data.0,
                    longitude: data.1
                }
            }

            println!("Didnt found the item so throwing error");

            return Response::Error(Error::NoData);
        }

        Response::Error(Error::InvalidRequest)
    }
}