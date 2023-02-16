use super::Call;

const AGGREGATE_FUNC_SIG: &str = "252dba42";
const PARAM_COUNT_LEN: usize = 32;
const ADDR_LEN: usize = 32;
const ZEROES: &str = "000000000000000000000000";
const DATA_PART_LEN: usize = 64;

fn aggregate(calls: &[Call]) -> String {
    let param_count_len = format!("{:064x}", PARAM_COUNT_LEN);
    let param_count = format!("{:064x}", calls.len());

    let aggregated = calls
        .iter()
        .map(|call| {
            let data_len = call.call_data.len() / 2;
            let padding = vec!["0"; (DATA_PART_LEN - data_len) * 2].join("");

            format!(
                "{ZEROES}{:x}{:064x}{data_len:064x}{}{padding}",
                call.target, DATA_PART_LEN, call.call_data
            )
        })
        .reduce(|a, b| format!("{a}{b}"))
        .unwrap_or_default();

    let mut offset = String::new();
    let mut factor = calls.len();

    for _ in 0..calls.len() {
        offset += &format!("{:064x}", factor * 32);
        factor += 5;
    }

    let data = String::new()
        + AGGREGATE_FUNC_SIG
        + &param_count_len
        + &param_count
        + &offset
        + &aggregated;

    data
}

#[cfg(test)]
mod test {
    use crate::evm::{
        jsonrpc::{
            contract::{call_contract, erc20_call, multicall::aggregate, Call},
            GetProvider,
        },
        EvmChain,
    };
    use rusty_gate_common::address;

    #[test]
    fn aggregate_test() {
        let func_sig = "252dba42";
        let param_count_length = "0000000000000000000000000000000000000000000000000000000000000020";
        let param_count = "0000000000000000000000000000000000000000000000000000000000000001";
        let offset = "0000000000000000000000000000000000000000000000000000000000000020";
        let target_address = "000000000000000000000000458691c1692cd82facfb2c5127e36d63213448a8";
        let data_part_length = "0000000000000000000000000000000000000000000000000000000000000040";
        let data_length = "0000000000000000000000000000000000000000000000000000000000000024";
        let data = "70a0823100000000000000000000000014ddfe8ea7ffc338015627d160ccaf99e8f16dd300000000000000000000000000000000000000000000000000000000";

        let call_data_2 = String::new()
            + func_sig
            + param_count_length
            + param_count
            + offset
            + target_address
            + data_part_length
            + data_length
            + data;

        let call = erc20_call(
            address!("0x458691c1692cd82facfb2c5127e36d63213448a8"),
            address!("0x14DDFE8EA7FFc338015627D160ccAf99e8F16Dd3"),
        );

        let call_data_1 = aggregate(&vec![call]);

        assert_eq!(call_data_1, call_data_2);
    }

    #[tokio::test]
    async fn multicall_balance() {
        let client = reqwest::Client::new();
        let chain = EvmChain::Ethereum;

        let call_1 = erc20_call(
            address!("0x458691c1692cd82facfb2c5127e36d63213448a8"),
            address!("0xE43878Ce78934fe8007748FF481f03B8Ee3b97DE"),
        );

        let call_2 = erc20_call(
            address!("0x458691c1692cd82facfb2c5127e36d63213448a8"),
            address!("0x14DDFE8EA7FFc338015627D160ccAf99e8F16Dd3"),
        );

        let aggregated = aggregate(&vec![call_1, call_2]);

        let call = Call {
            target: chain.provider().unwrap().contract,
            call_data: aggregated,
        };

        let res = call_contract(&client, chain, call).await.unwrap();

        assert_eq!(res, "".to_string());
    }
}
