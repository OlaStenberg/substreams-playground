use std::ops::{Add, Div, Mul};
use std::str::FromStr;
use bigdecimal::{BigDecimal, One, Zero};
use num_bigint::BigUint;
use pad::PadStr;
use prost::DecodeError;
use substreams::{log, proto, state};
use crate::pb;

pub const WBNB_ADDRESS: &str = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
pub const BUSD_WBNB_PAIR: &str = "0x58f876857a02d6762e0101bb5c46a8c1ed44dc16";
pub const USDT_WBNB_PAIR: &str = "0x16b9a82891338f9ba80e2d6970fdda79d1eb0dae";
pub const BUSD_PRICE_KEY: &str = "price:0xe9e7cea3dedca5984780bafc599bd69add087d56:0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
pub const USDT_PRICE_KEY: &str = "price:0x55d398326f99059ff775485246999027b3197955:0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";

const WHITELIST_TOKENS: [&str; 6] = [
    "0xe9e7cea3dedca5984780bafc599bd69add087d56", // BUSD
    "0x55d398326f99059ff775485246999027b3197955", // USDT
    "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d", // USDC
    "0x23396cf899ca06c4472205fc903bdb4de249d6fc", // UST
    "0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c", // BTCB
    "0x2170ed0880ac9a755fd29b2688956bd959f933f8", // WETH
];

pub fn is_pair_created_event(sig: String) -> bool {
    /* keccak value for PairCreated(address,address,address,uint256) */
    return sig == "0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9";
}

pub fn is_new_pair_sync_event(sig: String) -> bool {
    /* keccak value for Sync(uint112,uint112) */
    return sig == "1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1";
}

pub fn convert_token_to_decimal(amount: &[u8], decimals: u64) -> BigDecimal {
    let big_uint_amount = BigUint::from_bytes_be(amount);
    let big_float_amount = BigDecimal::from_str(big_uint_amount.to_string().as_str()).unwrap().with_prec(100);

    return divide_by_decimals(big_float_amount, decimals);
}

pub fn get_token_price(bf0: BigDecimal, bf1: BigDecimal) -> BigDecimal {
    return bf0.div(bf1).with_prec(100);
}

pub fn generate_tokens_key(token0: String, token1: String) -> String {
    if token0 > token1 {
        return format!("{}:{}", token1, token0);
    }
    return format!("{}:{}", token0, token1);
}

// not sure about the & in front of reserve
pub fn compute_usd_price(reserve: &pb::pcs::Reserve, reserves_store_idx: u32) -> BigDecimal {
    let busd_bnb_reserve_big_decimal;
    let usdt_bnb_reserve_big_decimal;

    match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, format!("reserve:{}:{}", BUSD_WBNB_PAIR, WBNB_ADDRESS)) {
        None => busd_bnb_reserve_big_decimal = zero_big_decimal(),
        Some(reserve_bytes) =>  {
            // return zero_big_decimal(); // was this breaking ?
            busd_bnb_reserve_big_decimal = decode_reserve_bytes_to_big_decimal(reserve_bytes);
            log::println(format!("busd_bnb_reserve_big_decimal: {:?}", busd_bnb_reserve_big_decimal));
        }
    }

    match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, format!("reserve:{}:{}", USDT_WBNB_PAIR, WBNB_ADDRESS)) {
        None => usdt_bnb_reserve_big_decimal = zero_big_decimal(),
        Some(reserve_bytes) => {
            usdt_bnb_reserve_big_decimal = decode_reserve_bytes_to_big_decimal(reserve_bytes);
            log::println(format!("usdt_bnb_reserve_big_decimal: {:?}", usdt_bnb_reserve_big_decimal));
        }
    }

    let mut total_liquidity_bnb = zero_big_decimal();
    total_liquidity_bnb = total_liquidity_bnb.clone().add(busd_bnb_reserve_big_decimal.clone());
    total_liquidity_bnb = total_liquidity_bnb.clone().add(usdt_bnb_reserve_big_decimal.clone());

    let zero = zero_big_decimal();

    if total_liquidity_bnb.eq(&zero) {
        return zero
    }

    if busd_bnb_reserve_big_decimal.clone().eq(&zero) {
        return match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, USDT_PRICE_KEY.to_string()) {
            None => zero,
            Some(reserve_bytes) => {
                log::println(format!("decode_reserve_bytes_to_big_decimal busd_bnb_reserve_big_decimal: {:?}", busd_bnb_reserve_big_decimal));
                decode_reserve_bytes_to_big_decimal(reserve_bytes)
            }
        }
    } else if usdt_bnb_reserve_big_decimal.clone().eq(&zero) {
        return match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, BUSD_PRICE_KEY.to_string()) {
            None => zero,
            Some(reserve_bytes) => {
                log::println(format!("decode_reserve_bytes_to_big_decimal usdt_bnb_reserve_big_decimal: {:?}", usdt_bnb_reserve_big_decimal));
                decode_reserve_bytes_to_big_decimal(reserve_bytes)
            }
        }
    }

    // both found and not equal to zero, average out
    let busd_weight = busd_bnb_reserve_big_decimal.div(total_liquidity_bnb.clone()).with_prec(100);
    let usdt_weight = usdt_bnb_reserve_big_decimal.div(total_liquidity_bnb).with_prec(100);

    let busd_price = match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, USDT_PRICE_KEY.to_string()) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => {
            log::println(format!("decode_reserve_bytes_to_big_decimal busd_price"));
            decode_reserve_bytes_to_big_decimal(reserve_bytes)
        }
    };

    let usdt_price = match state::get_at(reserves_store_idx, reserve.log_ordinal as i64, BUSD_PRICE_KEY.to_string()) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => {
            log::println(format!("decode_reserve_bytes_to_big_decimal usdt_price"));
            decode_reserve_bytes_to_big_decimal(reserve_bytes)
        }
    };

    let busd_price_over_weight = busd_price.mul(busd_weight).with_prec(100);
    let usdt_price_over_weight = usdt_price.mul(usdt_weight).with_prec(100);

    let mut usd_price = zero_big_decimal();
    usd_price = usd_price.add(busd_price_over_weight);
    usd_price = usd_price.add(usdt_price_over_weight);

    usd_price
}

pub fn find_bnb_price_per_token(log_ordinal: &u64,
                                erc20_token_address: String,
                                pairs_store_idx: u32,
                                reserves_store_idx: u32) -> Option<BigDecimal> {
    if erc20_token_address.eq(&WBNB_ADDRESS) {
        return Option::Some(one_big_decimal())  // BNB price of a BNB is always 1
    }

    let direct_to_bnb_price = match state::get_last(reserves_store_idx, format!("price:{}:{}", WBNB_ADDRESS, erc20_token_address)) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => {
            zero_big_decimal()
            // log::println(format!("decode_reserve_bytes_to_big_decimal direct_to_bnb_price"));
            // decode_reserve_bytes_to_big_decimal(reserve_bytes)
        }
    };

    if !direct_to_bnb_price.eq(&zero_big_decimal()) {
        return Option::Some(direct_to_bnb_price)
    }

    // loop all whitelist for a matching pair
    for major_token in WHITELIST_TOKENS {
        let tiny_to_major_pair =
            match state::get_at(pairs_store_idx, *log_ordinal as i64, format!("tokens:{}", generate_tokens_key(erc20_token_address.clone(), major_token.to_string()))) {
                None => continue,
                Some(pair_bytes) => decode_pair_bytes(pair_bytes)
            };

        let major_to_bnb_price =
            match state::get_at(reserves_store_idx, *log_ordinal as i64, format!("price:{}:{}", major_token, WBNB_ADDRESS)) {
                None => continue,
                Some(reserve_bytes) => {
                    log::println(format!("decode_reserve_bytes_to_big_decimal major_to_bnb_price"));
                    decode_reserve_bytes_to_big_decimal(reserve_bytes)
                }
            };

        let tiny_to_major_price =
            match state::get_at(reserves_store_idx, *log_ordinal as i64, format!("price:{}:{}", erc20_token_address, major_token)) {
                None => continue,
                Some(reserve_bytes) => {
                    log::println(format!("decode_reserve_bytes_to_big_decimal tiny_to_major_price"));
                    decode_reserve_bytes_to_big_decimal(reserve_bytes)
                }
            };

        let major_reserve =
            match state::get_at(reserves_store_idx, *log_ordinal as i64, format!("reserve:{}:{}", tiny_to_major_pair.erc20_token0.unwrap().address, major_token)) {
                None => continue,
                Some(reserve_bytes) => {
                    log::println(format!("decode_reserve_bytes_to_big_decimal major_reserve"));
                    decode_reserve_bytes_to_big_decimal(reserve_bytes)
                }
            };

        let bnb_reserve_in_major_pair = major_to_bnb_price.clone().mul(major_reserve);
        // We're checking for half of it, because `reserves_bnb` would have both sides in it.
        // We could very well check the other reserve's BNB value, would be a bit more heavy, but we can do it.
        if bnb_reserve_in_major_pair.le(&BigDecimal::from_str("5").unwrap()) {
            continue; // Not enough liquidity
        }

        return Some(tiny_to_major_price.mul(major_to_bnb_price));
    }

    return None
}

pub fn zero_big_decimal() -> BigDecimal {
    BigDecimal::zero().with_prec(100)
}

fn one_big_decimal() -> BigDecimal {
    BigDecimal::one().with_prec(100)
}

fn divide_by_decimals(big_float_amount: BigDecimal, decimals: u64) -> BigDecimal{
    let bd = BigDecimal::from_str("1".pad_to_width_with_char((decimals + 1) as usize, '0').as_str()).unwrap().with_prec(100);
    return big_float_amount.div(bd).with_prec(100)
}

fn decode_pair_bytes(mut pair_bytes: Vec<u8>) -> pb::pcs::Pair {
    log::println(format!("byte array: {:?}", pair_bytes));
    let pair_from_store_decoded_result: Result<pb::pcs::Pair, DecodeError> = proto::decode_ptr(pair_bytes.as_mut_ptr(), pair_bytes.len());
    // need to take care of the use case if the data doesnt exist. ie we will be getting a decode error
    if pair_from_store_decoded_result.is_err() {
        log::println(format!("error occurred when decoding pair"));
        return pb::pcs::Pair { // fixme: this is temporary, we are gonna need to fix this...
            address: "".to_string(),
            erc20_token0: None,
            erc20_token1: None,
            creation_transaction_id: "".to_string(),
            block_num: 0,
            log_ordinal: 0
        }
    }
    return pair_from_store_decoded_result.unwrap();
}

fn decode_reserve_bytes_to_big_decimal(mut reserve_bytes: Vec<u8>) -> BigDecimal {
    log::println(format!("reserve_bytes: {:?}", reserve_bytes));
    log::println(format!("reserve_bytes.as_mut_ptr(): {:?}", reserve_bytes.as_mut_ptr()));
    log::println(format!("reserve_bytes.len(): {:?}", reserve_bytes.len()));

    let reserve_from_store_decoded: pb::pcs::Reserve = proto::decode_ptr(reserve_bytes.as_mut_ptr(), reserve_bytes.len()).unwrap();
    log::println(format!("reserve_from_store_decoded reserve0 ok: {:?}", reserve_from_store_decoded.reserve0));

    return BigDecimal::from_str(reserve_from_store_decoded.reserve0.as_str()).unwrap().with_prec(100);
}