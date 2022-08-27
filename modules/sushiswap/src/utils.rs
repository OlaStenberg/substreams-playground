use std::ops::{Add, Div, Mul};
use std::str;
use std::str::FromStr;

use bigdecimal::{BigDecimal, One, Zero};
use num_bigint::BigUint;
use pad::PadStr;
use substreams::{proto, store};

use crate::pb;

pub const NATIVE_ADDRESS: &str = "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270";
pub const WMATIC_USDC_PAIR: &str = "0xcd353f79d9fade311fc3119b841e1f456b54e858";
pub const WMATIC_USDT_PAIR: &str = "0x55ff76bffc3cdd9d5fdbbc2ece4528ecce45047e";
pub const BUSD_PRICE_KEY: &str =
    "price:0xe9e7cea3dedca5984780bafc599bd69add087d56:0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
pub const USDT_PRICE_KEY: &str =
    "price:0x55d398326f99059ff775485246999027b3197955:0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";

pub const SUSHI_ADDRESS: &str = "0x0b3f868e0be5597d5db7feb59e1cadbb0fdda50a";
pub const WETH_ADDRESS: &str = "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619";
pub const WBTC_ADDRESS: &str = "0x1bfd67037b42cf73acf2047067bd4f2c47d9bfd6";
pub const USDC_ADDRESS: &str = "0x2791bca1f2de4661ed88a30c99a7a9449aa84174";
pub const USDT_ADDRESS: &str = "0xc2132d05d31c914a87c6611c10748aeb04b58e8f";
pub const DAI_ADDRESS: &str = "0x8f3cf7ad23cd3cadbd9735aff958023239c6a063";
pub const AAVE_ADDRESS: &str = "0xd6df932a45c0f255f85145f286ea0b292b21c90b";
pub const FRAX_ADDRESS: &str = "0x45c32fa6df82ead1e2ef74d17b76547eddfaff89";
pub const BCT_ADDRESS: &str = "0x2f800db0fdb5223b3c3f354886d907a671414a7f";
pub const AURUM_ADDRESS: &str = "0x34d4ab47bee066f361fa52d792e69ac7bd05ee23";
pub const MSU_ADDRESS: &str = "0xe8377a076adabb3f9838afb77bee96eac101ffb1";
pub const DMAGIC_ADDRESS: &str = "0x61daecab65ee2a1d5b6032df030f3faa3d116aa7";
pub const NDEFI_ADDRESS: &str = "0xd3f07ea86ddf7baebefd49731d7bbd207fedc53b";


const WHITELIST_TOKENS: [&str; 14] = [
    NATIVE_ADDRESS,
    SUSHI_ADDRESS,
    WETH_ADDRESS,
    WBTC_ADDRESS,
    USDC_ADDRESS,
    USDT_ADDRESS,
    DAI_ADDRESS,
    AAVE_ADDRESS,
    FRAX_ADDRESS,
    BCT_ADDRESS,
    AURUM_ADDRESS,
    MSU_ADDRESS,
    DMAGIC_ADDRESS,
    NDEFI_ADDRESS,
];

pub fn convert_token_to_decimal(amount: &[u8], decimals: &u64) -> BigDecimal {
    let big_uint_amount = BigUint::from_bytes_be(amount);
    let big_float_amount = BigDecimal::from_str(big_uint_amount.to_string().as_str())
        .unwrap()
        .with_prec(100);

    return divide_by_decimals(big_float_amount, decimals);
}

pub fn get_token_price(bf0: BigDecimal, bf1: BigDecimal) -> BigDecimal {
    return bf0.div(bf1).with_prec(100);
}

pub fn generate_tokens_key(token0: &str, token1: &str) -> String {
    if token0 > token1 {
        return format!("{}:{}", token1, token0);
    }
    return format!("{}:{}", token0, token1);
}

// not sure about the & in front of reserve
pub fn compute_usd_price(reserves_store: &store::StoreGet, reserve: &pb::pcs::Reserve) -> BigDecimal {
    let wmatic_usdc_reserve_big_decimal;
    let wmatic_usdt_reserve_big_decimal;

    match reserves_store.get_at(
        reserve.log_ordinal,
        &format!("reserve:{}:{}", WMATIC_USDC_PAIR, NATIVE_ADDRESS),
    ) {
        None => wmatic_usdc_reserve_big_decimal = zero_big_decimal(),
        Some(reserve_bytes) => {
            wmatic_usdc_reserve_big_decimal = decode_reserve_bytes_to_big_decimal(reserve_bytes)
        }
    }

    match reserves_store.get_at(
        reserve.log_ordinal,
        &format!("reserve:{}:{}", WMATIC_USDT_PAIR, NATIVE_ADDRESS),
    ) {
        None => wmatic_usdt_reserve_big_decimal = zero_big_decimal(),
        Some(reserve_bytes) => {
            wmatic_usdt_reserve_big_decimal = decode_reserve_bytes_to_big_decimal(reserve_bytes)
        }
    }

    let mut total_liquidity_native = zero_big_decimal();
    total_liquidity_native = total_liquidity_native
        .clone()
        .add(wmatic_usdc_reserve_big_decimal.clone());
    total_liquidity_native = total_liquidity_native
        .clone()
        .add(wmatic_usdt_reserve_big_decimal.clone());

    let zero = zero_big_decimal();

    if total_liquidity_native.eq(&zero) {
        return zero;
    }

    // if wmatic_usdc_reserve_big_decimal.eq(&zero) {
    //     return match reserves_store.get_at(
    //         reserve.log_ordinal,
    //         &USDT_PRICE_KEY.to_string(),
    //     ) {
    //         None => zero,
    //         Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
    //     };
    // } else if wmatic_usdt_reserve_big_decimal.eq(&zero) {
    //     return match reserves_store.get_at(
    //         reserve.log_ordinal,
    //         &BUSD_PRICE_KEY.to_string(),
    //     ) {
    //         None => zero,
    //         Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
    //     };
    // }

    // both found and not equal to zero, average out
    let usdc_weight = wmatic_usdc_reserve_big_decimal
        .div(total_liquidity_native.clone())
        .with_prec(100);
    let usdt_weight = wmatic_usdt_reserve_big_decimal
        .div(total_liquidity_native)
        .with_prec(100);

    let busd_price = match reserves_store.get_at(
        reserve.log_ordinal,
        &USDT_PRICE_KEY.to_string(),
    ) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
    };

    let usdt_price = match reserves_store.get_at(
        reserve.log_ordinal,
        &BUSD_PRICE_KEY.to_string(),
    ) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
    };

    let usdc_price_over_weight = busd_price.mul(usdc_weight).with_prec(100);
    let usdt_price_over_weight = usdt_price.mul(usdt_weight).with_prec(100);

    let mut usd_price = zero_big_decimal();
    usd_price = usd_price.add(usdc_price_over_weight);
    usd_price = usd_price.add(usdt_price_over_weight);

    usd_price
}

pub fn find_native_price_per_token(
    log_ordinal: &u64,
    erc20_token_address: &str,
    pairs_store: &store::StoreGet,
    reserves_store: &store::StoreGet,
) -> Option<BigDecimal> {
    if erc20_token_address.eq(NATIVE_ADDRESS) {
        return Some(one_big_decimal()); // native price of a native is always 1
    }

    let direct_to_native_price = match reserves_store.get_last(
        &format!("price:{}:{}", NATIVE_ADDRESS, erc20_token_address),
    ) {
        None => zero_big_decimal(),
        Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
    };

    if direct_to_native_price.ne(&zero_big_decimal()) {
        return Some(direct_to_native_price);
    }

    // loop all whitelist for a matching pair
    for major_token in WHITELIST_TOKENS {
        let tiny_to_major_pair = match pairs_store.get_at(
            *log_ordinal,
            &format!(
                "tokens:{}",
                generate_tokens_key(erc20_token_address, major_token)
            ),
        ) {
            None => continue,
            Some(pair_bytes) => decode_pair_bytes(pair_bytes),
        };

        let major_to_native_price = match reserves_store.get_at(
            *log_ordinal,
            &format!("price:{}:{}", major_token, NATIVE_ADDRESS),
        ) {
            None => continue,
            Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
        };

        let tiny_to_major_price = match reserves_store.get_at(
            *log_ordinal,
            &format!("price:{}:{}", erc20_token_address, major_token),
        ) {
            None => continue,
            Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes),
        };

        let major_reserve =
            //todo: not sure about tiny_to_major_pair.erc20_token0.addr, maybe its the token1 ?
            match reserves_store.get_at(*log_ordinal, &format!("reserve:{}:{}", tiny_to_major_pair, major_token)) {
                None => continue,
                Some(reserve_bytes) => decode_reserve_bytes_to_big_decimal(reserve_bytes)
            };

        let native_reserve_in_major_pair = major_to_native_price.clone().mul(major_reserve);
        // We're checking for half of it, because `reserves_native` would have both sides in it.
        // We could very well check the other reserve's native value, would be a bit more heavy, but we can do it.
        if native_reserve_in_major_pair.le(&BigDecimal::from_str("5").unwrap()) {
            // todo: little or big ?
            continue; // Not enough liquidity
        }

        return Some(tiny_to_major_price.mul(major_to_native_price));
    }

    return None;
}

pub fn zero_big_decimal() -> BigDecimal {
    BigDecimal::zero().with_prec(100)
}

pub fn compute_amount_total(amount1: String, amount2: String) -> BigDecimal {
    let amount1_bd: BigDecimal = BigDecimal::from_str(amount1.as_str()).unwrap();
    let amount2_bd: BigDecimal = BigDecimal::from_str(amount2.as_str()).unwrap();

    amount1_bd.add(amount2_bd)
}

pub fn get_last_token(tokens: &store::StoreGet, token_address: &str) -> pb::tokens::Token {
    proto::decode(&tokens.get_last(&format!("token:{}", token_address)).unwrap())
        .unwrap()
}

fn one_big_decimal() -> BigDecimal {
    BigDecimal::one().with_prec(100)
}

fn divide_by_decimals(big_float_amount: BigDecimal, decimals: &u64) -> BigDecimal {
    let bd = BigDecimal::from_str(
        "1".pad_to_width_with_char((*decimals + 1) as usize, '0')
            .as_str(),
    )
    .unwrap()
    .with_prec(100);
    return big_float_amount.div(bd).with_prec(100);
}

fn decode_pair_bytes(pair_bytes: Vec<u8>) -> String {
    let pair_from_store_decoded = str::from_utf8(pair_bytes.as_slice()).unwrap();
    return pair_from_store_decoded.to_string();
}

fn decode_reserve_bytes_to_big_decimal(reserve_bytes: Vec<u8>) -> BigDecimal {
    let reserve_from_store_decoded = str::from_utf8(reserve_bytes.as_slice()).unwrap();
    return BigDecimal::from_str(reserve_from_store_decoded)
        .unwrap()
        .with_prec(100);
}
