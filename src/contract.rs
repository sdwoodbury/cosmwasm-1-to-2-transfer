#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, GetBalanceResponse, GetOwnerResponse, GetSendFeeResponse, InstantiateMsg, QueryMsg,
};
use crate::state::{State, BALANCES, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cosmwasm-1-to-2-transfer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        owner: info.sender.clone(),
        send_fee: msg.send_fee,
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;
    let owner_balance = Uint128::from(0u32);
    BALANCES.save(deps.storage, info.sender.clone(), &owner_balance)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("send_fee", msg.send_fee.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env, // mostly used for block height at this point
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            recipient_a,
            recipient_b,
        } => execute_transfer(deps, info, &recipient_a, &recipient_b),
        ExecuteMsg::Withdraw { amount } => execute_withdraw(deps, info, amount),
    }
}

pub fn execute_transfer(
    deps: DepsMut,
    info: MessageInfo,
    recipient_a: &str,
    recipient_b: &str,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;

    // validate funds: should be a vector with one element: the usei coin
    if info.funds.is_empty() {
        return Err(ContractError::CustomError {
            val: "please send usei".into(),
        });
    }

    if info.funds.len() != 1 {
        return Err(ContractError::CustomError {
            val: "please only send usei".into(),
        });
    }

    let funds = if info.funds[0].denom == "usei" {
        info.funds[0].amount
    } else {
        return Err(ContractError::CustomError {
            val: format!(
                "invalid denomination {}. please send usei",
                info.funds[0].denom
            ),
        });
    };

    // ensure balance (minus the transfer fee) is even (instructions say to divide money evenly. requires an even number) and nonzero
    if funds <= state.send_fee {
        return Err(ContractError::CustomError {
            val: format!(
                "not enough funds. please send an even number of usei + a fee of {}",
                state.send_fee
            ),
        });
    }

    // ensure the funds can be divided evenly
    // to_send is guaranteed to be nonzero
    let to_send = funds - state.send_fee;
    if to_send % Uint128::from(2u32) != Uint128::from(0u32) {
        return Err(ContractError::CustomError {
            val: format!(
                "not enough funds. please send an even number of usei + a fee of {}",
                state.send_fee
            ),
        });
    }

    // calculate the amount to give to each account
    // half is guaranteed to be nonzero
    let half = to_send / Uint128::from(2u32);

    // create accounts if not exist and credit accounts
    // can only move DepsMut once so have to do this in a loop :(
    let accounts = vec![recipient_a, recipient_b];
    for account in accounts {
        let addr = deps.api.addr_validate(account)?;
        if !BALANCES.has(deps.storage, addr.clone()) {
            BALANCES.save(deps.storage, addr, &half)?;
        } else {
            let balance = BALANCES.load(deps.storage, addr.clone())?;
            let new_balance = match Uint128::checked_add(balance, half) {
                Ok(r) => r,
                Err(_) => {
                    return Err(ContractError::CustomError {
                        val: "balance overflow occured".into(),
                    })
                }
            };

            // delete empty balance unless the account is the owner
            // keeping the owner balance simplifies the logic in execute_transfer => can always assume the account exists.
            if new_balance == Uint128::from(0u32) && addr != state.owner {
                BALANCES.remove(deps.storage, addr);
            } else {
                BALANCES.save(deps.storage, addr, &new_balance)?;
            }
        }
    }

    // update the owner balance
    let owner_balance = BALANCES.load(deps.storage, state.owner.clone())?;
    let new_owner_balance = match Uint128::checked_add(owner_balance, state.send_fee) {
        Ok(r) => r,
        Err(_) => {
            // this is pretty crappy. would make the contract unusable until the owner withdraws funds
            return Err(ContractError::CustomError {
                val: "owner balance overflow occured".into(),
            });
        }
    };
    BALANCES.save(deps.storage, state.owner, &new_owner_balance)?;

    // emit event
    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("recipientA", half)
        .add_attribute("recipientB", half))
}

pub fn execute_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // ensure account exists
    if !BALANCES.has(deps.storage, info.sender.clone()) {
        return Err(ContractError::Unauthorized {});
    }
    // check balance
    let balance = BALANCES.load(deps.storage, info.sender.clone())?;
    if amount > balance {
        return Err(ContractError::CustomError {
            val: "insufficient funds".into(),
        });
    }

    // deduct balance
    let new_balance = balance - amount;
    BALANCES.save(deps.storage, info.sender.clone(), &new_balance)?;

    // send coins
    let mut res = Response::new();
    res = res.add_message(BankMsg::Send {
        to_address: info.sender.into(),
        amount: coins(amount.u128(), "usei"),
    });

    Ok(res.add_attribute("action", "withdraw"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner {} => to_binary(&query_owner(deps)?),
        QueryMsg::GetSendFee {} => to_binary(&query_send_fee(deps)?),
        QueryMsg::GetBalance { account } => to_binary(&query_balance(deps, &account)?),
    }
}

fn query_owner(deps: Deps) -> StdResult<GetOwnerResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(GetOwnerResponse { owner: state.owner })
}

fn query_send_fee(deps: Deps) -> StdResult<GetSendFeeResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(GetSendFeeResponse {
        fee: state.send_fee,
    })
}

fn query_balance(deps: Deps, account: &str) -> StdResult<GetBalanceResponse> {
    let addr = deps.api.addr_validate(account)?;

    let balance = if BALANCES.has(deps.storage, addr.clone()) {
        // returns error if key isn't present. have to check `has` first
        BALANCES.load(deps.storage, addr)?
    } else {
        Uint128::from(0u32)
    };
    Ok(GetBalanceResponse { balance })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    /*#[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: GetCountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    } */
}
