use crate::{
    evm::{
        jsonrpc::{
            contract::multicall::{aggregate, parse_multicall_result},
            create_payload, GetProvider, RpcError, RpcResponse, ETH_BALANCE_DIVIDER,
        },
        EvmChain,
    },
    rpc_error,
};
use guild_common::Scalar;
use primitive_types::{H160 as Address, U256};
use std::str::FromStr;

mod multicall;

const ZEROES: &str = "000000000000000000000000";
const FUNC_DECIMALS: &str = "313ce567";
const FUNC_ETH_BALANCE: &str = "4d2301cc";
const FUNC_BALANCE_OF: &str = "70a08231";
const FUNC_BALANCE_OF_ID: &str = "6352211e";
const FUNC_ERC1155: &str = "00fdd58e";
const FUNC_ERC1155_BATCH: &str = "4e1273f4";

#[derive(Clone, Debug)]
pub struct Call {
    pub target: Address,
    pub call_data: String,
}

async fn call_contract(
    client: &reqwest::Client,
    chain: EvmChain,
    call: Call,
) -> Result<String, RpcError> {
    let provider = chain.provider()?;

    let params = format!(
        "[
            {{
                \"to\"   : \"{:?}\",
                \"data\" : \"0x{}\"
            }},
            \"latest\"
        ]",
        call.target, call.call_data
    );
    let payload = create_payload("eth_call", params, 1);

    let res: RpcResponse = client
        .post(&provider.rpc_url)
        .body(payload)
        .send()
        .await?
        .json()
        .await?;

    Ok(res.result)
}

pub async fn get_eth_balance_batch(
    client: &reqwest::Client,
    chain: EvmChain,
    user_addresses: &[Address],
) -> Result<Vec<Scalar>, RpcError> {
    let target = chain.provider()?.contract;

    let calls = user_addresses
        .iter()
        .map(|addr| Call {
            target,
            call_data: format!("{FUNC_ETH_BALANCE}{ZEROES}{addr:x}"),
        })
        .collect::<Vec<Call>>();

    let call = Call {
        target,
        call_data: aggregate(&calls),
    };

    let res = call_contract(client, chain, call).await?;
    let balances = parse_multicall_result(&res)?
        .iter()
        .map(|balance| balance / ETH_BALANCE_DIVIDER)
        .collect();

    Ok(balances)
}

pub async fn get_erc20_decimals(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
) -> Result<U256, RpcError> {
    let call = Call {
        target: token_address,
        call_data: FUNC_DECIMALS.to_string(),
    };
    let decimals = call_contract(client, chain, call).await?;

    rpc_error!(U256::from_str(&decimals))
}

fn erc20_call(token_address: Address, user_address: Address) -> Call {
    Call {
        target: token_address,
        call_data: format!("{FUNC_BALANCE_OF}{ZEROES}{user_address:x}"),
    }
}

pub async fn get_erc20_balance(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    user_address: Address,
) -> Result<Scalar, RpcError> {
    let call = erc20_call(token_address, user_address);
    let balance = call_contract(client, chain, call).await?;
    let decimals = get_erc20_decimals(client, chain, token_address).await?;
    let divider = 10_u128.pow(decimals.as_u32()) as Scalar;

    let balance = rpc_error!(U256::from_str(&balance))?;

    Ok((balance.as_u128() as Scalar) / divider)
}

pub async fn get_erc20_balance_batch(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    user_addresses: &[Address],
) -> Result<Vec<Scalar>, RpcError> {
    let calls = user_addresses
        .iter()
        .map(|user_address| erc20_call(token_address, *user_address))
        .collect::<Vec<Call>>();

    let call = Call {
        target: chain.provider()?.contract,
        call_data: aggregate(&calls),
    };

    let res = call_contract(client, chain, call).await?;

    let balances = parse_multicall_result(&res)?
        .iter()
        .map(|balance| balance / ETH_BALANCE_DIVIDER)
        .collect();

    Ok(balances)
}

pub fn erc721_call(token_address: Address, user_address: Address) -> Call {
    erc20_call(token_address, user_address)
}

fn erc721_id_call(token_address: Address, id: U256) -> Call {
    Call {
        target: token_address,
        call_data: format!("{FUNC_BALANCE_OF_ID}{id:064x}"),
    }
}

pub async fn get_erc721_balance(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    token_id: Option<U256>,
    user_address: Address,
) -> Result<Scalar, RpcError> {
    let balance = match token_id {
        Some(id) => {
            let call = erc721_id_call(token_address, id);
            let addr = call_contract(client, chain, call).await?;

            (addr[26..] == format!("{user_address:x}")) as u8 as Scalar
        }
        None => {
            let call = erc721_call(token_address, user_address);
            let balance = call_contract(client, chain, call).await?;

            rpc_error!(U256::from_str(&balance))?.as_u128() as Scalar
        }
    };

    Ok(balance)
}

pub async fn get_erc721_balance_batch(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    user_addresses: &[Address],
) -> Result<Vec<Scalar>, RpcError> {
    let calls = user_addresses
        .iter()
        .map(|user_address| erc721_call(token_address, *user_address))
        .collect::<Vec<Call>>();

    let call = Call {
        target: chain.provider()?.contract,
        call_data: aggregate(&calls),
    };

    let res = call_contract(client, chain, call).await?;

    parse_multicall_result(&res)
}

fn erc1155_call(token_address: Address, id: U256, user_address: Address) -> Call {
    Call {
        target: token_address,
        call_data: format!("{FUNC_ERC1155}{ZEROES}{user_address:x}{id:064x}"),
    }
}

pub async fn get_erc1155_balance(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    token_id: U256,
    user_address: Address,
) -> Result<Scalar, RpcError> {
    let call = erc1155_call(token_address, token_id, user_address);
    let res = call_contract(client, chain, call).await?;

    let balance = rpc_error!(U256::from_str(&res))?.as_u128() as Scalar;

    Ok(balance)
}

pub async fn get_erc1155_balance_batch(
    client: &reqwest::Client,
    chain: EvmChain,
    token_address: Address,
    token_id: U256,
    user_addresses: &[Address],
) -> Result<Vec<Scalar>, RpcError> {
    let addresses = user_addresses
        .iter()
        .map(|user_address| format!("{ZEROES}{user_address:x}"))
        .collect::<String>();

    let len = 64;
    let count = user_addresses.len();
    let offset = (count + 3) * 32;
    let ids = vec![format!("{token_id:064x}"); count].join("");

    let call_data = format!(
        "{FUNC_ERC1155_BATCH}{len:064x}{offset:064x}{count:064x}{addresses}{count:064x}{ids}",
    );

    let call = Call {
        target: token_address,
        call_data,
    };

    let res = call_contract(client, chain, call).await?;

    let balances = res
        .trim_start_matches("0x")
        .chars()
        .collect::<Vec<char>>()
        .chunks(64)
        .skip(2)
        .map(|c| {
            let balance = c.iter().collect::<String>();

            rpc_error!(U256::from_str(&balance).map(|value| value.as_u128() as Scalar))
        })
        .collect::<Vec<Result<Scalar, RpcError>>>();

    balances.into_iter().collect()
}

#[cfg(all(test, feature = "nomock"))]
mod test {
    use crate::evm::{common::*, jsonrpc::get_erc20_decimals, EvmChain};
    use guild_common::address;
    use primitive_types::U256;

    #[tokio::test]
    async fn rpc_get_erc20_decimals() {
        let client = reqwest::Client::new();
        let chain = EvmChain::Ethereum;
        let token_1 = ERC20_ADDR;
        let token_2 = "0x343e59d9d835e35b07fe80f5bb544f8ed1cd3b11";
        let token_3 = "0xaba8cac6866b83ae4eec97dd07ed254282f6ad8a";
        let token_4 = "0x0a9f693fce6f00a51a8e0db4351b5a8078b4242e";

        let decimals_1 = get_erc20_decimals(&client, chain, address!(token_1))
            .await
            .unwrap();
        let decimals_2 = get_erc20_decimals(&client, chain, address!(token_2))
            .await
            .unwrap();
        let decimals_3 = get_erc20_decimals(&client, chain, address!(token_3))
            .await
            .unwrap();
        let decimals_4 = get_erc20_decimals(&client, chain, address!(token_4))
            .await
            .unwrap();

        assert_eq!(decimals_1, U256::from(18));
        assert_eq!(decimals_2, U256::from(9));
        assert_eq!(decimals_3, U256::from(24));
        assert_eq!(decimals_4, U256::from(5));
    }
}
