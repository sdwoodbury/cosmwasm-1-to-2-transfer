use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, export_schema_with_title, remove_schemas, schema_for};

use cosmwasm_1_to_2_transfer::msg::{
    ExecuteMsg, GetBalanceResponse, GetOwnerResponse, GetSendFeeResponse, InstantiateMsg, QueryMsg,
};
use cosmwasm_1_to_2_transfer::state::State;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(State), &out_dir);
    export_schema_with_title(
        &schema_for!(GetBalanceResponse),
        &out_dir,
        "GetBalanceResponse",
    );
    export_schema_with_title(&schema_for!(GetOwnerResponse), &out_dir, "GetOwnerResponse");
    export_schema_with_title(
        &schema_for!(GetSendFeeResponse),
        &out_dir,
        "GetSendFeeResponse",
    );
}
