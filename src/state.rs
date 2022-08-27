use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    /// every send incurs a small fee, which is sent to the owner of the contract
    /// this contract only supports the usei coin
    pub send_fee: Uint128,
}

pub const STATE: Item<State> = Item::new("state");
/// stores the withdrawable balance of every account that this contract was used to send coins to
pub const BALANCES: Map<Addr, Uint128> = Map::new("balances");
