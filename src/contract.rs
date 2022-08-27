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
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            val: "the creator shouldn't send money to this contract".into(),
        });
    }
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

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
            val: "funds <= fee".into(),
        });
    }

    // ensure the funds can be divided evenly
    // to_send is guaranteed to be nonzero
    let to_send = funds - state.send_fee;
    if to_send % Uint128::from(2u32) != Uint128::from(0u32) {
        return Err(ContractError::CustomError {
            val: format!(
                "invalid funds. please send an even number of usei + a fee of {}",
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

            // delete empty balance
            if new_balance == Uint128::from(0u32) {
                BALANCES.remove(deps.storage, addr);
            } else {
                BALANCES.save(deps.storage, addr, &new_balance)?;
            }
        }
    }

    // send fee
    let mut res = Response::new();
    res = res
        .add_message(BankMsg::Send {
            to_address: state.owner.into(),
            amount: coins(state.send_fee.u128(), "usei"),
        })
        .add_attribute("action", "transfer")
        .add_attribute("recipient_a", half)
        .add_attribute("recipient_b", half);
    Ok(res)
}

pub fn execute_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if !info.funds.is_empty() {
        return Err(ContractError::CustomError {
            val: "no funds required".into(),
        });
    }
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
    use cosmwasm_std::{coin, coins, from_binary, CosmosMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            send_fee: Uint128::from(1u32),
        };

        // negative path - initializing the contract with money
        let info = mock_info("creator", &coins(1000, "usei"));
        let res = instantiate(deps.as_mut(), mock_env(), info, msg.clone());
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("shouldn't send")),
            _ => assert!(false),
        };

        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // check owner
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetOwner {}).unwrap();
        let value: GetOwnerResponse = from_binary(&res).unwrap();
        assert_eq!("creator", value.owner);

        // check send_fee
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetSendFee {}).unwrap();
        let value: GetSendFeeResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u32), value.fee);

        // check balance of nonexistent account
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "random".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0u32), value.balance);
    }

    #[test]
    fn send_coins_negative_path() {
        // init the contract
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            send_fee: Uint128::from(1u32),
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // negative path: send the wrong type of coin
        let info = mock_info("sender_a", &coins(1, "BTC"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("invalid denomination")),
            _ => assert!(false),
        };

        // negative path: send multiple types of coin
        let to_send = vec![coin(1, "usei"), coin(1, "usei")];
        let info = mock_info("sender_a", &to_send);
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.as_str() == "please only send usei"),
            _ => assert!(false),
        };

        // negative path: send no coins
        let info = mock_info("sender_a", &[]);
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.as_str() == "please send usei"),
            _ => assert!(false),
        };

        // negative path: send the wrong number of coins (odd number greater than fee)
        // 4 - fee (1) = 3, which is not divisible by 2
        let info = mock_info("sender_a", &coins(4, "usei"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("invalid funds")),
            _ => assert!(false),
        };

        // negative path: send the wrong number of coins (just send the fee)
        let info = mock_info("sender_a", &coins(1, "usei"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("funds <= fee")),
            _ => assert!(false),
        };

        // negative path: send the wrong number of coins (zero)
        let info = mock_info("sender_a", &coins(0, "usei"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b");
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("funds <= fee")),
            _ => assert!(false),
        };
    }

    #[test]
    fn send_coins() {
        // init the contract
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            send_fee: Uint128::from(1u32),
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // send coins to the same address
        let info = mock_info("sender_a", &coins(3, "usei"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_a").unwrap();
        // verify the creator was paid
        assert!(res.messages.len() == 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".into(),
                amount: coins(1, "usei"),
            })
        );

        // send coins to different addresses
        let info = mock_info("sender_a", &coins(7, "usei"));
        let res = execute_transfer(deps.as_mut(), info, "recipient_b", "recipient_c").unwrap();
        // verify the creator was paid
        assert!(res.messages.len() == 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator".into(),
                amount: coins(1, "usei"),
            })
        );

        // query balances of recipients
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_a".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(2u32), value.balance);

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_b".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(3u32), value.balance);

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_c".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(3u32), value.balance);
    }

    #[test]
    fn withdraw_coins() {
        // init the contract
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            send_fee: Uint128::from(1u32),
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // send coins
        let info = mock_info("sender_a", &coins(7, "usei"));
        execute_transfer(deps.as_mut(), info, "recipient_a", "recipient_b").unwrap();

        // query balance
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_a".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(3u32), value.balance);

        // withdraw using account not listed
        let info = mock_info("random", &[]);
        let res = execute_withdraw(deps.as_mut(), info, Uint128::from(1u32));
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::Unauthorized {} => {}
            _ => assert!(false),
        };

        // withdraw too many
        let info = mock_info("recipient_a", &[]);
        let res = execute_withdraw(deps.as_mut(), info, Uint128::from(4u32));
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("insufficient funds")),
            _ => assert!(false),
        };

        // send money with withdrawal request
        let info = mock_info("recipient_a", &coins(1, "usei"));
        let res = execute_withdraw(deps.as_mut(), info, Uint128::from(4u32));
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::CustomError { val } => assert!(val.contains("no funds required")),
            _ => assert!(false),
        };

        // withdraw less than total
        let info = mock_info("recipient_a", &[]);
        let res = execute_withdraw(deps.as_mut(), info, Uint128::from(2u32)).unwrap();

        // verify the recipient was paid
        assert!(res.messages.len() == 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "recipient_a".into(),
                amount: coins(2, "usei"),
            })
        );

        // query balance
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_a".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(1u32), value.balance);

        // withdraw remaining
        let info = mock_info("recipient_a", &[]);
        let res = execute_withdraw(deps.as_mut(), info, Uint128::from(1u32)).unwrap();

        // verify the recipient was paid
        assert!(res.messages.len() == 1);
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "recipient_a".into(),
                amount: coins(1, "usei"),
            })
        );

        // query balance
        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetBalance {
                account: "recipient_a".into(),
            },
        )
        .unwrap();
        let value: GetBalanceResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::from(0u32), value.balance);
    }
}
