//! This crate is an experimental implementation of Blind Off-chain
//! lightweight transactions (BOLT).
//!
//! It builds on academic work done by Ian Miers and Matthew Green -
//! https://eprint.iacr.org/2016/701.
//!
//! Libbolt relies on BN curves at 128-bit security, as implemented in
//! [`bn module`](https://github.com/zcash-hackworks/bn).
//!
#![allow(non_snake_case)]
#![feature(extern_prelude)]

#![cfg_attr(all(test, feature = "unstable"), feature(test))]
#[cfg(all(test, feature = "unstable"))] extern crate test;

extern crate bn;
extern crate rand;
extern crate rand_core;
extern crate bincode;
extern crate sodiumoxide;
extern crate rustc_serialize;
extern crate secp256k1;
extern crate time;
extern crate merlin;
extern crate bulletproofs;
extern crate curve25519_dalek;
extern crate sha2;

extern crate serde;
extern crate serde_with;

extern crate libc;


use std::fmt;
use std::str;
use bn::{Group, Fr, G1, G2, Gt};
use bincode::SizeLimit::Infinite;
use bincode::rustc_serialize::{encode, decode};
use sodiumoxide::randombytes;
use sodiumoxide::crypto::hash::sha512;
use sha2::Sha512;
use std::collections::HashMap;
use curve25519_dalek::digest::*;
use curve25519_dalek::scalar::Scalar;
use merlin::Transcript;
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};

use serde::{Serialize, Deserialize};

pub mod prf;
pub mod sym;
pub mod ote;
pub mod clsigs;
pub mod commit_scheme;
pub mod clproto;
pub mod serialization_wrappers;

const E_MIN: i32 = 1;
const E_MAX: i32 = 255; // TODO: should be 2^32 - 1

pub fn debug_elem_in_hex(prefix: &str, r: &Fr) {
    let encoded: Vec<u8> = encode(&r, Infinite).unwrap();
    print!("{} (hex) = 0x", prefix);
    for e in encoded.iter() {
        print!("{:x}", e);
    }
    print!("\n");
}

pub fn debug_g1_in_hex(prefix: &str, g: &G1) {
    let encoded: Vec<u8> = encode(&g, Infinite).unwrap();
    print!("{} (hex) = 0x", prefix);
    for e in encoded.iter() {
        print!("{:x}", e);
    }
    print!("\n");
}

pub fn debug_g2_in_hex(prefix: &str, g: &G2) {
    let encoded: Vec<u8> = encode(&g, Infinite).unwrap();
    print!("{} (hex) = 0x", prefix);
    for e in encoded.iter() {
        print!("{:x}", e);
    }
    print!("\n");
}

pub fn debug_gt_in_hex(prefix: &str, g: &Gt) {
    let encoded: Vec<u8> = encode(&g, Infinite).unwrap();
    print!("{} (hex) = 0x", prefix);
    for e in encoded.iter() {
        print!("{:x}", e);
    }
    print!("\n");
}


struct HexSlice<'a>(&'a [u8]);

impl<'a> HexSlice<'a> {
    fn new<T>(data: &'a T) -> HexSlice<'a>
        where T: ?Sized + AsRef<[u8]> + 'a
    {
        HexSlice(data.as_ref())
    }
}

impl<'a> fmt::LowerHex for HexSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            // Decide if you want upper- or lowercase results,
            // padding the values to two characters, spaces
            // between bytes, etc.
            write!(f, "{:x}", byte)?;
        }
        Ok(())
    }
}

impl<'a> fmt::UpperHex for HexSlice<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0 {
            // Decide if you want upper- or lowercase results,
            // padding the values to two characters, spaces
            // between bytes, etc.
            write!(f, "{:X}", byte)?;
        }
        Ok(())
    }
}

pub fn print(g: &G1) -> String {
    let c_vec: Vec<u8> = encode(g, Infinite).unwrap();
    let mut c_s = String::new();
    for x in c_vec.iter() {
        c_s = format!("{}{:x}", c_s, x);
    }

    return c_s;
}

pub fn print_length(commit: &commit_scheme::Commitment) {
    let c_vec: Vec<u8> = encode(&commit.c, Infinite).unwrap();
    println!("Length of G2: {}", c_vec.len());
    let r_vec: Vec<u8> = encode(&commit.r, Infinite).unwrap();
    println!("Length of FR: {}", r_vec.len());
}

// OLD RefundMessage
//impl<'a> RefundMessage<'a> {
//    pub fn new(_c_id: Fr, _index: i32) -> RefundMessage<'a> {
//        RefundMessage {
//            prefix: "refund", c_id: _c_id, index: _index,
//        }
//    }
//
//    pub fn hash(&self) -> Fr {
//        let mut input_buf = Vec::new();
//        input_buf.extend_from_slice(self.prefix.as_bytes());
//        let c_id_vec: Vec<u8> = encode(&self.c_id, Infinite).unwrap();
//        // encode cId in the vector
//        input_buf.extend(c_id_vec);
//        // encode the balance as a hex string
//        let b = format!("{:x}", self.index);
//        input_buf.extend_from_slice(b.as_bytes());
//        // TODO: add the ck vector (l-bit key)
////        let mut in_str = String::new();
////        for y in input_buf.iter() {
////            in_str = format!("{}{:x}", in_str, y);
////        }
////        println!("input_buf: {}", in_str);
//
//        // hash the inputs via SHA256
//        let sha2_digest = sha512::hash(input_buf.as_slice());
//        // println!("hash: {:?}", sha2_digest);
//        // let h = format!("{:x}", HexSlice::new(&sha2_digest));
//        let mut hash_buf: [u8; 64] = [0; 64];
//        hash_buf.copy_from_slice(&sha2_digest[0..64]);
//        return Fr::interpret(&hash_buf);
//    }
//}

// spend message (for unidirectional scheme)
#[derive(Clone)]
pub struct SpendMessage<'a> {
    prefix: &'a str,
    j: i32,
    s: G1,
    u: G1,
    pi: Proof,
    ck: sym::SymKey
}

impl<'a> SpendMessage<'a> {
    pub fn new(_j: i32, _s: G1, _u: G1, _pi: Proof, _ck: sym::SymKey) -> SpendMessage<'a> {
        SpendMessage {
            prefix: "spend", j: _j, s: _s, u: _u, pi: _pi, ck: _ck,
        }
    }

    pub fn hash(&self) -> Fr {
        // hash into a Fr element
        let rng = &mut rand::thread_rng();
        return Fr::random(rng);
    }
}

#[derive(Copy, Clone)]
pub struct Message {
    sk: clsigs::SecretKey, // the secret key for the signature scheme (Is it possible to make this a generic field?)
    k1: Fr, // seed 1 for PRF
    k2: Fr, // seed 2 for PRF
    balance: i32 // the balance for the user
}

impl Message {
    pub fn new(_sk: clsigs::SecretKey, _k1: Fr, _k2: Fr, _balance: i32) -> Message {
        Message {
            sk: _sk, k1: _k1, k2: _k2, balance: _balance,
        }
    }

    pub fn hash(&self) -> Vec<Fr> {
        let input_buf = self.sk.encode();
        let mut v: Vec<Fr> = Vec::new();

        v.push(convert_to_fr(&input_buf));
        // encode k1 in the vector
        v.push(self.k1.clone());
        v.push(self.k2.clone());
        // convert the balance into a Fr
        let bal = Fr::from_str(self.balance.to_string().as_str()).unwrap();
        v.push(bal);

        return v;
    }
}

////////////////////////////////// Utilities //////////////////////////////////

pub fn concat_to_vector(output: &mut Vec<u8>, t: &G2) {
    let t_vec: Vec<u8> = encode(t, Infinite).unwrap();
    output.extend(t_vec);
}

#[derive(Copy, Clone)]
pub struct Proof {
    T: G1,
    c: Fr,
    s1: Fr,
    s2: Fr
}

pub fn hash_g1_to_fr(x: &G1) -> Fr {
    let x_vec: Vec<u8> = encode(&x, Infinite).unwrap();
    let sha2_digest = sha512::hash(x_vec.as_slice());

    let mut hash_buf: [u8; 64] = [0; 64];
    hash_buf.copy_from_slice(&sha2_digest[0..64]);
    return Fr::interpret(&hash_buf);
}

pub fn hash_pub_key_to_fr(wpk: &secp256k1::PublicKey) -> Fr {
    let x_slice = wpk.serialize_uncompressed();
    let sha2_digest = sha512::hash(&x_slice);

    let mut hash_buf: [u8; 64] = [0; 64];
    hash_buf.copy_from_slice(&sha2_digest[0..64]);
    return Fr::interpret(&hash_buf);
}

pub fn compute_pub_key_fingerprint(wpk: &secp256k1::PublicKey) -> String {
    let x_slice = wpk.serialize();
    let sha2_digest = sha512::hash(&x_slice);
    let h = format!("{:x}", HexSlice::new(&sha2_digest[0..16]));
    return h;
}

pub fn hash_buffer_to_fr<'a>(prefix: &'a str, buf: &[u8; 64]) -> Fr {
    let mut input_buf = Vec::new();
    input_buf.extend_from_slice(prefix.as_bytes());
    input_buf.extend_from_slice(buf);

    let sha2_digest = sha512::hash(&input_buf.as_slice());

    let mut hash_buf: [u8; 64] = [0; 64];
    hash_buf.copy_from_slice(&sha2_digest[0..64]);
    return Fr::interpret(&hash_buf);
}

fn convert_to_fr(input_buf: &Vec<u8>) -> Fr {
    // hash the inputs via SHA256
    let sha2_digest = sha512::hash(input_buf.as_slice());
    // println!("hash: {:?}", sha2_digest);
    // let h = format!("{:x}", HexSlice::new(&sha2_digest));
    let mut hash_buf: [u8; 64] = [0; 64];
    hash_buf.copy_from_slice(&sha2_digest[0..64]);
    return Fr::interpret(&hash_buf);
}

fn convert_str_to_fr<'a>(input: &'a str) -> Fr {
    let mut input_buf = Vec::new();
    input_buf.extend_from_slice(input.as_bytes());
    return convert_to_fr(&input_buf);
}

fn convert_int_to_fr(value: i32) -> Fr {
    if value > 0 {
        return Fr::from_str(value.to_string().as_str()).unwrap();
    } else {
        // negative value
        let value2 = value * -1;
        return -Fr::from_str(value2.to_string().as_str()).unwrap();
    }
}

// refund message
#[derive(Clone, Serialize, Deserialize)]
pub struct RefundMessage {
    pub msgtype: String, // purpose type of message
    #[serde(deserialize_with = "serialization_wrappers::deserialize_public_key")]
    pub wpk: secp256k1::PublicKey,
    pub balance: usize, // the balance
    #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable_option", deserialize_with = "serialization_wrappers::deserialize_optional_fr")]
    pub r: Option<Fr>, // randomness from customer wallet
    pub rt: Option<clsigs::SignatureD> // refund token
}

impl RefundMessage {
    pub fn new(_msgtype: String, _wpk: secp256k1::PublicKey,
               _balance: usize, _r: Option<Fr>, _rt: Option<clsigs::SignatureD>) -> RefundMessage {
        RefundMessage {
            msgtype: _msgtype, wpk: _wpk, balance: _balance, r: _r, rt: _rt
        }
    }

    pub fn hash(&self) -> Vec<Fr> {
        let mut v: Vec<Fr> = Vec::new();
        let mut input_buf = Vec::new();
        input_buf.extend_from_slice(self.msgtype.as_bytes());
        v.push(convert_to_fr(&input_buf));

        v.push(hash_pub_key_to_fr(&self.wpk));

        // encoee the balance as a hex string
        let b = format!("{:x}", self.balance);
        let mut b_buf = Vec::new();
        b_buf.extend_from_slice(b.as_bytes());
        v.push(convert_to_fr(&b_buf));

        //let r_vec: Vec<u8> = encode(&self.r, Infinite).unwrap();
        if !self.r.is_none() {
            v.push(self.r.unwrap().clone());
        }

        if !self.rt.is_none() {
            let rt = {
                &self.rt.clone()
            };
            let rt_ref = rt.as_ref();
            v.push(rt_ref.unwrap().hash(&self.msgtype));
        }

        return v;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RevokedMessage {
    pub msgtype: String,
    #[serde(deserialize_with = "serialization_wrappers::deserialize_public_key")]
    pub wpk: secp256k1::PublicKey,
    #[serde(serialize_with = "serialization_wrappers::serialize_fixed_byte_array_option", deserialize_with = "serialization_wrappers::deserialize_optional_fixed_64_byte_array")]
    pub sig: Option<[u8; 64]> // represents revocation token serialized compact bytes
}

impl RevokedMessage {
    pub fn new(_msgtype: String, _wpk: secp256k1::PublicKey, _sig: Option<[u8; 64]>) -> RevokedMessage {
        RevokedMessage {
            msgtype: _msgtype, wpk: _wpk, sig: _sig
        }
    }

    pub fn hash(&self) -> Vec<Fr> {
        let mut v: Vec<Fr> = Vec::new();
        let mut input_buf = Vec::new();
        input_buf.extend_from_slice(self.msgtype.as_bytes());
        v.push(convert_to_fr(&input_buf));

        v.push(hash_pub_key_to_fr(&self.wpk));

        if !self.sig.is_none() {
            v.push(hash_buffer_to_fr(&self.msgtype, &self.sig.unwrap()));
        }
        return v;
    }

    // return a message digest (32-bytes)
    pub fn hash_to_slice(&self) -> [u8; 32] {
        let mut input_buf = Vec::new();
        input_buf.extend_from_slice(self.msgtype.as_bytes());
        input_buf.extend_from_slice(&self.wpk.serialize_uncompressed());
        let sha2_digest = sha512::hash(input_buf.as_slice());
        // println!("hash: {:?}", sha2_digest);
        // let h = format!("{:x}", HexSlice::new(&sha2_digest));
        let mut hash_buf: [u8; 32] = [0; 32];
        hash_buf.copy_from_slice(&sha2_digest[0..32]);
        return hash_buf;
    }
}

////////////////////////////////// Utilities //////////////////////////////////

/////////////////////////////// Unidirectional ////////////////////////////////

pub mod unidirectional {
    use std::fmt;
    use std::collections::HashMap;
    use rand::{Rng, thread_rng};
    use rand_core::RngCore;
    use bn::{Group, Fr, G2};
    use sym;
    use commit_scheme;
    use clsigs;
    use clproto;
    use Message;
    use sodiumoxide::randombytes;

    #[derive(Clone)]
    pub struct CustomerWallet {
        sk: clsigs::SecretKeyD, // the secret key for the signature scheme (Is it possible to make this a generic field?)
        cid: Fr, // channel Id
        wpk: secp256k1::PublicKey, // signature verification key
        wsk: secp256k1::SecretKey // signature signing key
    }

    pub struct PublicParams {
        cl_mpk: clsigs::PublicParams,
        l: usize
    }

    pub struct ChannelToken {
        w_com: commit_scheme::Commitment,
        pk: clsigs::PublicKey
    }

    pub struct CustSecretKey {
        sk: clsigs::SecretKey, // the secret key for the signature scheme
        k1: Fr, // seed 1 for PRF
        k2: Fr, // seed 2 for PRF
        r: Fr, // random coins for commitment scheme
        balance: i32, // the balance for the user
        ck_vec: Vec<sym::SymKey>
    }

    pub struct MerchSecretKey {
        sk: clsigs::SecretKey,
        balance: i32
    }

    pub struct InitCustomerData {
        channel_token: ChannelToken,
        csk: CustSecretKey
    }

    pub struct InitMerchantData {
        channel_token: clsigs::PublicKey,
        csk: MerchSecretKey
    }

    pub struct PubKeyMap {
        wpk: secp256k1::PublicKey,
        revoke_token: Option<secp256k1::Signature>
    }

    pub struct ChannelState {
        keys: HashMap<String, PubKeyMap>,
        R: i32,
        tx_fee: i32,
        pub name: String,
        pub cid: Fr,
        pub pay_init: bool,
        pub channel_established: bool,
        pub third_party: bool
    }

    pub fn setup() -> PublicParams {
        let cl_mpk = clsigs::setup_d();
        let l = 4;
        let pp = PublicParams { cl_mpk: cl_mpk, l: l };
        return pp;
    }

    pub fn keygen(pp: &PublicParams) -> clsigs::KeyPairD {
        let keypair = clsigs::keygen_d(&pp.cl_mpk, pp.l);
        return keypair;
    }

    ///
    /// init_customer - takes as input the public params, channel state, commitment params, keypair,
    /// and initial balance for customer and merchant. Generate initial customer channel token,
    /// and wallet commitment.
    ///
    pub fn init_customer(pp: &PublicParams, cm_pk: commit_scheme::CSParams,
                         b0_customer: i32, b0_merchant: i32,
                         keypair: &clsigs::KeyPair) -> InitCustomerData {
        sym::init_mod();
        let rng = &mut rand::thread_rng();
        // pick two distinct seeds
        let l = 256;
        let k1 = Fr::random(rng);
        let k2 = Fr::random(rng);
        let r = Fr::random(rng);
        let msg = Message::new(keypair.sk, k1, k2, b0_customer);

        let mut ck_vec: Vec<sym::SymKey> = Vec::new();
        // generate the vector ck of sym keys
        for i in 1 .. b0_customer {
            let ck = sym::keygen(l);
            ck_vec.push(ck);
        }

        let w_com = commit_scheme::commit(&cm_pk, &msg.hash(), r);
        let t_c = ChannelToken { w_com: w_com, pk: keypair.pk };
        let csk_c = CustSecretKey { sk: keypair.sk, k1: k1, k2: k2, r: r, balance: b0_customer, ck_vec: ck_vec };
        return InitCustomerData { channel_token: t_c, csk: csk_c };
    }

    ///
    /// init_merchant - takes as input the public params, merchant balance and keypair.
    /// Generates merchant data which consists of channel token and merchant wallet.
    ///
    pub fn init_merchant(pp: &PublicParams, b0_merchant: i32, keypair: &clsigs::KeyPair) -> InitMerchantData {
        let csk_m = MerchSecretKey { sk: keypair.sk, balance: b0_merchant };
        return InitMerchantData { channel_token: keypair.pk, csk: csk_m };
    }

    ///
    /// establish_customer_phase1 - takes as input the public params, customer wallet and
    /// common public bases from merchant. Generates a PoK of the committed values in the
    /// new wallet.
    ///
    pub fn establish_customer_phase1(pp: &PublicParams, c_data: &InitCustomerData,
                                     pub_bases: &Vec<G2>) -> clproto::ProofCV {
        unimplemented!();
//        // set sk_0 to random bytes of length l
//        // let sk_0 = random_bytes(pp.l);
//        let buf_len: usize = pp.l_bits as usize;
//        let mut sk0 = vec![0; buf_len];
//        randombytes::randombytes_into(&mut sk0);
//
//        let pi1 = create_nizk_proof_one(csk_c.sk, csk_c.k1, csk_c.k2, );
    }

    ///
    /// establish_merchant_phase2 - takes as input the public params, channel state, initial
    /// merchant wallet and PoK of committed values from the customer. Generates a blinded
    /// signature over the contents of the customer's wallet.
    ///
    pub fn establish_merchant_phase2(pp: &PublicParams, state: &mut ChannelState, m_data: &InitMerchantData,
                                     proof: &clproto::ProofCV) -> clsigs::SignatureD {
        unimplemented!();
    }

    ///
    /// establish_customer_final - takes as input the public params, merchant's verification key,
    /// customer wallet and blinded signature obtained from merchant. Add the returned
    /// blinded signature to the wallet.
    ///
    pub fn establish_customer_final(pp: &PublicParams, pk_m: &clsigs::PublicKeyD,
                                    w: &mut CustomerWallet, sig: clsigs::SignatureD) -> bool {
        unimplemented!();
    }

    // TODO: add pay protocol api, channel disput algs, etc
}

/////////////////////////////// Unidirectional ////////////////////////////////

/////////////////////////////// Bidirectional ////////////////////////////////
pub mod bidirectional {
    use std::fmt;
    use rand::{rngs::OsRng, Rng};
    use rand_core::RngCore;
    use bn::{Group, Fr, G1, G2, Gt};
    use commit_scheme;
    use clsigs;
    use clproto;
    use sodiumoxide;
    use secp256k1;
    use RefundMessage;
    use RevokedMessage;
    use HashMap;
    use hash_pub_key_to_fr;
    use debug_elem_in_hex;
    use debug_gt_in_hex;
    use convert_str_to_fr;
    use convert_int_to_fr;
    use compute_pub_key_fingerprint;
    use E_MIN;
    use E_MAX;
    //use hash_buffer_to_fr;
    //use debug_g2_in_hex;
    //use convert_to_fr;
    use bulletproofs;
    use sodiumoxide::crypto::hash::sha512;
    use sha2::Sha512;
    use curve25519_dalek::scalar::Scalar;
    use curve25519_dalek::ristretto::RistrettoPoint;
    use merlin::Transcript;
    use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
    use bincode::rustc_serialize::encode;
    use bincode::SizeLimit::Infinite;

    use serialization_wrappers;
    use serde::{Serialize, Deserialize};

    fn print_secret_vector(x: &Vec<Fr>) {
        for i in 0 .. x.len() {
            let msg = format!("x[{}] => ", i);
            debug_elem_in_hex(&msg, &x[i]);
        }
    }

    fn print_public_bases_vector(g: &Vec<Gt>) {
        for i in 0 .. g.len() {
            let msg = format!("g[{}] => ", i);
            debug_gt_in_hex(&msg, &g[i]);
        }
    }

    #[derive(Serialize, Deserialize)]
    pub struct PublicParams {
        pub cl_mpk: clsigs::PublicParams,
        l: usize, // messages for commitment

        #[serde(serialize_with = "serialization_wrappers::serialize_bullet_proof", deserialize_with = "serialization_wrappers::deserialize_bullet_proof" )]
        bp_gens: bulletproofs::BulletproofGens,
        range_proof_bits: usize,
        extra_verify: bool // extra verification for certain points in the establish/pay protocol
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct ChannelToken {
        w_com: commit_scheme::Commitment,
        pk: clsigs::PublicKeyD,
        third_party_pay: bool
    }

    // proof of wallet signature, blind signature on wallet and common params for NIZK
    #[derive(Clone, Serialize, Deserialize)]
    pub struct CustomerWalletProof {
        proof_cv: clproto::ProofCV, // proof of knowledge of committed values
        proof_vs: clproto::ProofVS, // proof of knowledge of valid signature

        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_g_two")]
        bal_com: G2, // old balance commitment
        blind_sig: clsigs::SignatureD, // a blind signature
        common_params: clproto::CommonParams, // common params for NIZK
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct CustomerWallet {
        sk: clsigs::SecretKeyD, // the secret key for the signature scheme (Is it possible to make this a generic field?)
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_fr")]
        cid: Fr, // channel Id
        #[serde(deserialize_with = "serialization_wrappers::deserialize_public_key")]
        wpk: secp256k1::PublicKey, // signature verification key
        #[serde(deserialize_with = "serialization_wrappers::deserialize_secret_key")]
        wsk: secp256k1::SecretKey, // signature signing key
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_fr")]
        h_wpk: Fr,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_fr")]
        r: Fr, // random coins for commitment scheme
        pub balance: i32, // the balance for the user
        merchant_balance: i32,
        signature: Option<clsigs::SignatureD>,
        // proof of signature on wallet contents in zero-knowledge
        proof: Option<CustomerWalletProof>,
        refund_token: Option<clsigs::SignatureD>
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct MerchSecretKey {
        sk: clsigs::SecretKeyD, // merchant signing key
        pub balance: i32
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct InitCustomerData {
        pub channel_token: ChannelToken,
        pub csk: CustomerWallet,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable_vec", deserialize_with = "serialization_wrappers::deserialize_g_two_vec")]
        pub bases: Vec<G2>,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct InitMerchantData {
        pub channel_token: clsigs::PublicKeyD,
        pub csk: MerchSecretKey,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable_vec", deserialize_with = "serialization_wrappers::deserialize_g_two_vec")]
        pub bases: Vec<G2>
    }

    // part of channel state
    #[derive(Clone, Serialize, Deserialize)]
    pub struct PubKeyMap {
        #[serde(deserialize_with = "serialization_wrappers::deserialize_public_key")]
        wpk: secp256k1::PublicKey,
        revoke_token: Option<secp256k1::Signature>
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct ChannelState {
        keys: HashMap<String, PubKeyMap>,
        R: i32,
        tx_fee: i32,
        pub name: String,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_fr")]
        pub cid: Fr,
        pub pay_init: bool,
        pub channel_established: bool,
        pub third_party: bool
    }

    impl ChannelState {
        pub fn new(name: String, third_party_support: bool)-> ChannelState {
            ChannelState {
                keys: HashMap::new(), // store wpks/revoke_tokens
                R: 0,
                tx_fee: 0,
                name: name.to_string(),
                cid: Fr::from_str("0").unwrap(),
                pay_init: false,
                channel_established: false,
                third_party: third_party_support
            }
        }

        pub fn generate_channel_id(&mut self, pk: &clsigs::PublicKeyD) {
            let pk_bytes = pk.encode();
            let sha2_digest = sha512::hash(&pk_bytes.as_slice());

            let mut hash_buf: [u8; 64] = [0; 64];
            hash_buf.copy_from_slice(&sha2_digest[0..64]);
            self.cid = Fr::interpret(&hash_buf);
        }

        pub fn set_channel_fee(&mut self, fee: i32) {
            self.tx_fee = fee;
        }

        pub fn get_channel_fee(&self) -> i32 {
            return self.tx_fee as i32;
        }
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct ChannelclosureC {
        pub message: RefundMessage,
        pub signature: clsigs::SignatureD
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct ChannelclosureM {
        pub message: RevokedMessage,
        pub signature: clsigs::SignatureD
    }

    // proof of valid balance
    #[derive(Clone, Serialize, Deserialize)]
    pub struct ProofVB {
        #[serde(deserialize_with = "serialization_wrappers::deserialize_range_proof")]
        range_proof: (bulletproofs::RangeProof, curve25519_dalek::ristretto::CompressedRistretto),
        #[serde(deserialize_with = "serialization_wrappers::deserialize_r_point")]
        value_commitment: RistrettoPoint
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct BalanceProof {
        third_party: bool,
        balance_increment: i32,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_g_two")]
        w_com_pr_pr: G2,
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_g_two")]
        old_bal_com: G2,
        vcom: Option<commit_scheme::Commitment>,
        proof_vcom: Option<clproto::ProofCV>,
        proof_vrange: Option<ProofVB>
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct PaymentProof {
        proof2a: clproto::ProofCV, // PoK of committed values in new wallet
        //proof2b: clproto::ProofCV, // PoK of committed values in old wallet (minus wpk)
        proof2c: clproto::ProofVS, // PoK of old wallet signature (that includes wpk)
        proof3: ProofVB, // range proof that balance - balance_inc is between (0, val_max)
        #[serde(serialize_with = "serialization_wrappers::serialize_generic_encodable", deserialize_with = "serialization_wrappers::deserialize_g_two")]
        old_com_base: G2,
        #[serde(deserialize_with = "serialization_wrappers::deserialize_public_key")]
        wpk: secp256k1::PublicKey, // verification key for old wallet
        wallet_sig: clsigs::SignatureD, // blinded signature for old wallet
        pub bal_proof: BalanceProof
    }

    #[derive(Serialize, Deserialize)]
    pub struct RevokeToken {
        message: RevokedMessage,
        #[serde(deserialize_with = "serialization_wrappers::deserialize_secp_signature")]
        pub signature: secp256k1::Signature
    }

    pub fn init() {
        sodiumoxide::init();
    }

    ///
    /// setup - generate public parameters for bidirectional payment channels
    ///
    pub fn setup(_extra_verify: bool) -> PublicParams {
        let cl_mpk = clsigs::setup_d();
        let l = 4;
        let n = 32; // bitsize: 32-bit (0, 2^32-1)
        let num_rand_values = 1;
        let generators = BulletproofGens::new(64, num_rand_values); // bitsize

        let pp = PublicParams { cl_mpk: cl_mpk, l: l, bp_gens: generators, range_proof_bits: n, extra_verify: _extra_verify };
        return pp;
    }

    ///
    /// keygen - takes as input public parameters and generates a digital signature keypair
    ///
    pub fn keygen(pp: &PublicParams) -> clsigs::KeyPairD {
        let keypair = clsigs::keygen_d(&pp.cl_mpk, pp.l);
        return keypair;
    }

    pub fn generate_commit_setup(pp: &PublicParams, pk: &clsigs::PublicKeyD) -> commit_scheme::CSParams {
        let g2 = pp.cl_mpk.g2.clone();
        let bases = pk.Z2.clone();
        let cm_csp = commit_scheme::setup(pp.l, bases, g2);
        return cm_csp;
    }

    ///
    /// init_customer - takes as input the public params, channel state, commitment params, keypair,
    /// and initial balance for customer and merchant. Generate initial customer channel token,
    /// and wallet commitment.
    ///
    pub fn init_customer(pp: &PublicParams, channel: &mut ChannelState, b0_customer: i32, b0_merchant: i32,
                         cm_csp: &commit_scheme::CSParams, keypair: &clsigs::KeyPairD) -> InitCustomerData {
        assert!(b0_customer >= 0);
        assert!(b0_merchant >= 0);
        let rng = &mut rand::thread_rng();
        // generate verification key and signing key (for wallet)
        let mut schnorr = secp256k1::Secp256k1::new();
        schnorr.randomize(rng);
        let (wsk, wpk) = schnorr.generate_keypair(rng);
        let h_wpk = hash_pub_key_to_fr(&wpk);
        // convert balance into Fr
        let b0 = convert_int_to_fr(b0_customer);
        // randomness for commitment
        let r = Fr::random(rng);
        // retrieve the channel id
        channel.generate_channel_id(&keypair.pk);
        let cid = channel.cid;
        // initial contents of wallet:
        // commitment, channel id, customer balance, hash of wpk (wallet ver/pub key)
        let mut x: Vec<Fr> = vec![r, cid, b0, h_wpk];
        // commitment of wallet values
        let w_com = commit_scheme::commit(&cm_csp,  &x, r);

        // construct channel token
        let t_c = ChannelToken { w_com: w_com, pk: keypair.pk.clone(), third_party_pay: channel.third_party };

        // construct customer wallet secret key plus other components
        let csk_c = CustomerWallet { sk: keypair.sk.clone(), cid: cid, wpk: wpk, wsk: wsk, h_wpk: h_wpk,
                                    r: r, balance: b0_customer, merchant_balance: b0_merchant,
                                    proof: None, signature: None, refund_token: None };

        return InitCustomerData { channel_token: t_c, csk: csk_c, bases: cm_csp.pub_bases.clone() };
    }

    ///
    /// init_merchant - takes as input the public params, merchant balance and keypair.
    /// Generates merchant data which consists of channel token and merchant wallet.
    ///
    pub fn init_merchant(pp: &PublicParams, b0_merchant: i32, keypair: &clsigs::KeyPairD) -> InitMerchantData {
        assert!(b0_merchant >= 0);
        let cm_csp = generate_commit_setup(&pp, &keypair.pk);
        let csk_m = MerchSecretKey { sk: keypair.sk.clone(), balance: b0_merchant };
        return InitMerchantData { channel_token: keypair.pk.clone(), csk: csk_m, bases: cm_csp.pub_bases };
    }

    ///
    /// establish_customer_phase1 - takes as input the public params, customer wallet and
    /// common public bases from merchant. Generates a PoK of the committed values in the
    /// new wallet.
    ///
    pub fn establish_customer_phase1(pp: &PublicParams, c_data: &InitCustomerData,
                                     pub_bases: &Vec<G2>) -> clproto::ProofCV {
        // obtain customer init data
        let t_c = &c_data.channel_token;
        let csk_c = &c_data.csk;
        let h_wpk = csk_c.h_wpk;
        let b0 = convert_int_to_fr(csk_c.balance);
        // collect secrets
        let mut x: Vec<Fr> = vec![t_c.w_com.r, csk_c.cid, b0, h_wpk ];
        // generate proof of knowledge for committed values
        let proof_1 = clproto::bs_gen_nizk_proof(&x, &pub_bases, t_c.w_com.c);
        return proof_1;
    }

    ///
    /// establish_merchant_phase2 - takes as input the public params, channel state, initial
    /// merchant wallet and PoK of committed values from the customer. Generates a blinded
    /// signature over the contents of the customer's wallet.
    ///
    pub fn establish_merchant_phase2(pp: &PublicParams, state: &mut ChannelState, m_data: &InitMerchantData,
                                     proof: &clproto::ProofCV) -> clsigs::SignatureD {
        // verifies proof (\pi_1) and produces signature on the committed values in the initial wallet
        let wallet_sig = clproto::bs_check_proof_and_gen_signature(&pp.cl_mpk, &m_data.csk.sk, &proof);
        state.channel_established = true;
        return wallet_sig;
    }

    ///
    /// establish_customer_final - takes as input the public params, merchant's verification key,
    /// customer wallet and blinded signature obtained from merchant. Add the returned
    /// blinded signature to the wallet.
    ///
    pub fn establish_customer_final(pp: &PublicParams, pk_m: &clsigs::PublicKeyD,
                                    w: &mut CustomerWallet, sig: clsigs::SignatureD) -> bool {
        if w.signature.is_none() {
            if pp.extra_verify {
                // customer can verify that merchant generated a correct signature on
                // the expected committed values
                let bal = convert_int_to_fr(w.balance);
                let mut x: Vec<Fr> = vec![w.r.clone(), w.cid.clone(), bal, w.h_wpk.clone()];
                assert!(clsigs::verify_d(&pp.cl_mpk, &pk_m, &x, &sig));
            }
            w.signature = Some(sig);
            //println!("establish_customer_final - verified merchant signature on initial wallet with {}", w.balance);
            return true;
        }
        // must be an old wallet
        return false;
    }
    ///// end of establish channel protocol


    ///
    /// pay_by_customer_phase1_precompute - takes as input the public params, channel token,
    /// merchant verification key, old customer wallet. Generates PoK of signature on previous wallet.
    ///
    pub fn pay_by_customer_phase1_precompute(pp: &PublicParams, t: &ChannelToken, pk_m: &clsigs::PublicKeyD, old_w: &mut CustomerWallet) {
        // generate proof of knowledge of valid signature on previous wallet
        // get channel id, balance, commitment randomness and wallet sig
        let cid = old_w.cid.clone();
        let old_r = &old_w.r;
        let old_wallet_sig = &old_w.signature;

        let wallet_sig = old_wallet_sig.clone().unwrap();
        // retrieve old balance
        let old_balance = convert_int_to_fr(old_w.balance);

        let old_h_wpk = old_w.h_wpk;
        let mut old_x: Vec<Fr> = vec![old_w.r.clone(), cid, old_balance, old_h_wpk];
        // retrieve the commitment scheme parameters based on merchant's PK
        let cm_csp = generate_commit_setup(&pp, &pk_m);
        // extract the portion of the commitment to the balance of the wallet
        let bal_index = 2;
        let old_w_bal_com = cm_csp.pub_bases[bal_index] * old_balance;

        // proof of committed values not including the old wpk since we are revealing it
        // to the merchant
        let index = 3;
        let old_w_com_pr = t.w_com.c - old_w_bal_com - (cm_csp.pub_bases[index] * old_h_wpk);
        // NOTE: the third argument represents the challenge that is included in the final proof structure
        let proof_old_cv = clproto::bs_gen_nizk_proof(&old_x, &cm_csp.pub_bases, old_w_com_pr);

        // generate the blind signature for the old wallet signature (we do not want to reveal this
        // to the merchant)
        let blind_sig = clproto::prover_generate_blinded_sig(&wallet_sig);
        // generate the common params necessary to execute the two party NIZK protocol
        // for verifying the signature
        let common_params = clproto::gen_common_params(&pp.cl_mpk, &pk_m, &wallet_sig);
        // generate the NIZK proof of valid signature based on the old wallet
        let proof_vs = clproto::vs_gen_nizk_proof(&old_x, &common_params, common_params.vs);

        // return the payment proof for the old wallet
        let old_iou_proof = CustomerWalletProof { proof_cv: proof_old_cv, proof_vs: proof_vs,
            bal_com: old_w_bal_com, blind_sig: blind_sig, common_params: common_params };
        old_w.proof = Some(old_iou_proof);
    }


    ///
    /// pay_by_customer_phase1 - takes as input the public params, channel state, channel token,
    /// merchant public keys, old wallet and balance increment. Generate a new wallet commitment
    /// PoK of the committed values in new wallet and PoK of old wallet. Return new channel token,
    /// new wallet (minus blind signature and refund token) and payment proof.
    ///
    pub fn pay_by_customer_phase1(pp: &PublicParams, channel: &ChannelState, t: &ChannelToken, pk_m: &clsigs::PublicKeyD,
                                  old_w: &CustomerWallet, balance_increment: i32) -> (ChannelToken, CustomerWallet, PaymentProof) {
        let mut rng = &mut rand::thread_rng();

        if old_w.proof.is_none() {
           panic!("You have not executed the pay_by_customer_phase1_precompute!");
        }
        let wallet_proof = old_w.proof.clone().unwrap();
        let bal = old_w.balance;

        // generate new keypair
        let mut schnorr = secp256k1::Secp256k1::new();
        schnorr.randomize(rng);
        let (wsk, wpk) = schnorr.generate_keypair(rng);
        let h_wpk = hash_pub_key_to_fr(&wpk);

        // new sample randomness r'
        let r_pr = Fr::random(rng);
        // retrieve the commitment scheme parameters based on merchant's PK
        let cm_csp = generate_commit_setup(&pp, &pk_m);
        // retrieve the current payment channel id
        let cid = old_w.cid.clone();
        // convert balance into Fr (B - e)
        let updated_balance = bal - balance_increment - channel.tx_fee;
        if updated_balance < 0 {
            panic!("pay_by_customer_phase1 - insufficient funds to make payment!");
        }
        // record the potential to payment
        let merchant_balance = old_w.merchant_balance + (balance_increment + channel.tx_fee);

        let updated_balance_pr = convert_int_to_fr(updated_balance);

        let mut new_wallet_sec: Vec<Fr> = vec![r_pr, cid, updated_balance_pr, h_wpk];
        // commitment of new wallet values
        let w_com = commit_scheme::commit(&cm_csp, &new_wallet_sec, r_pr);
        let w_com_bytes: Vec<u8> = encode(&w_com.c, Infinite).unwrap();

        // generate proof of knowledge for committed values in new wallet
        let mut proof_cv = clproto::bs_gen_nizk_proof(&new_wallet_sec, &cm_csp.pub_bases, w_com.c);
        let bal_index = 2; // index of balance
        // sending partial commitment that does not include the balance
        let w_com_pr_pr = proof_cv.C - (cm_csp.pub_bases[bal_index] * updated_balance_pr);
        let wpk_index = new_wallet_sec.len() - 1;

        // bullet proof integration here to generate the range proof
        let mut transcript = Transcript::new(b"BOLT Range Proof");
        let value = updated_balance as u64;
        let val_blinding = Scalar::hash_from_bytes::<Sha512>(&w_com_bytes);
        let pc_gens = PedersenGens::default();
        let range_proof = RangeProof::prove_single(&pp.bp_gens, &pc_gens, &mut transcript,
                                                   value, &val_blinding,
                                                   pp.range_proof_bits).unwrap();
        //let pg = &pp.range_proof_gens.pedersen_gens;
        let value_cm = pc_gens.commit(Scalar::from(value), val_blinding);

        let proof_rp = ProofVB { range_proof: range_proof, value_commitment: value_cm };

        let mut bal_proof;
        if t.third_party_pay {
            let r_inc = Fr::random(rng);
            let bal_inc_fr = -convert_int_to_fr(balance_increment + channel.tx_fee);
            let inc_vec: Vec<Fr> = vec![r_inc, bal_inc_fr];
            let mut v_com = commit_scheme::commit(&cm_csp, &inc_vec, r_inc);
            let proof_vcom = clproto::bs_gen_nizk_proof(&inc_vec, &cm_csp.pub_bases, v_com.c);

            // range proof that pay increment < payment max
            let v_com_bytes: Vec<u8> = encode(&proof_vcom.C, Infinite).unwrap();

            let mut inc_bal;
            let final_balance_increment = balance_increment + channel.tx_fee;
            if final_balance_increment < 0 {
                // negative value => convert to positive value
                assert!(final_balance_increment >= -E_MAX);
                inc_bal = -final_balance_increment as u64
            } else {
                // positive value
                inc_bal = final_balance_increment as u64;
            }
            let inc_blinding = Scalar::hash_from_bytes::<Sha512>(&v_com_bytes);
            let mut transcript1 = Transcript::new(b"Range Proof for Balance Increment");
            let pc_gens = PedersenGens::default();
            let inc_range_proof = RangeProof::prove_single(&pp.bp_gens, &pc_gens, &mut transcript1,
                                                       inc_bal, &inc_blinding,
                                                       pp.range_proof_bits).unwrap();
            //let inc_pg = &pp.range_proof_gens.pedersen_gens;
            let inc_cm = pc_gens.commit(Scalar::from(inc_bal), inc_blinding);

            let proof_vrange = ProofVB { range_proof: inc_range_proof, value_commitment: inc_cm };
            bal_proof = BalanceProof { third_party: true, vcom: Some(v_com),
                                       proof_vcom: Some(proof_vcom), proof_vrange: Some(proof_vrange),
                                       w_com_pr_pr: w_com_pr_pr, balance_increment: 0,
                                       old_bal_com: wallet_proof.bal_com,
                                     };
        } else {
            // balance_increment => // epsilon - payment increment/decrement
            // wallet_proof.bal_com => // old balance commitment
            bal_proof = BalanceProof { third_party: false, vcom: None,
                                       proof_vcom: None, proof_vrange: None,
                                       w_com_pr_pr: w_com_pr_pr, balance_increment: balance_increment,
                                       old_bal_com: wallet_proof.bal_com,
                                     };
        }

        // create payment proof which includes params to reveal wpk from old wallet
        let payment_proof = PaymentProof {
                                proof2a: proof_cv, // (1) PoK for committed values, wCom' (in new wallet)
                                //proof2b: wallet_proof.proof_cv, // PoK of committed values (minus h(wpk))
                                proof2c: wallet_proof.proof_vs, // PoK of signature on old wallet
                                proof3: proof_rp, // range proof that the updated_balance is within a public range
                                bal_proof: bal_proof,
                                old_com_base: cm_csp.pub_bases[wpk_index], // base Z
                                wpk: old_w.wpk.clone(), // showing public key for old wallet
                                wallet_sig: wallet_proof.blind_sig // blinded signature for old wallet
                            };
        // create new wallet structure (w/o signature or refund token)
        let t_c = ChannelToken { w_com: w_com, pk: t.pk.clone(), third_party_pay: t.third_party_pay };
        let csk_c = CustomerWallet { sk: old_w.sk.clone(), cid: cid, wpk: wpk, wsk: wsk, h_wpk: h_wpk,
                            r: r_pr, balance: updated_balance, merchant_balance: merchant_balance,
                            proof: None, signature: None, refund_token: None };
        return (t_c, csk_c, payment_proof);
    }

    ///
    /// pay_by_merchant_phase1 - takes as input the public params, channel state, payment proof
    /// and merchant keys. If proof is valid, then merchant returns the refund token
    /// (i.e., partially blind signature on IOU with updated balance)
    ///
    pub fn pay_by_merchant_phase1(pp: &PublicParams, mut state: &mut ChannelState, proof: &PaymentProof,
                                  m_data: &InitMerchantData) -> clsigs::SignatureD {
        let proof_cv = &proof.proof2a;
        //let proof_old_cv = &proof.proof2b;
        let proof_vs = &proof.proof2c;
        let bal_proof = &proof.bal_proof;
        let blinded_sig = &proof.wallet_sig;
        // get merchant keypair
        let pk_m = &m_data.channel_token;
        let sk_m = &m_data.csk.sk;

        // let's first confirm that proof of knowledge of signature on old wallet is valid
        let proof_vs_old_wallet = clproto::vs_verify_blind_sig(&pp.cl_mpk, &pk_m, &proof_vs, &blinded_sig);

//        // add specified wpk to make the proof valid
//        // NOTE: if valid, then wpk is indeed the wallet public key for the wallet
//        let new_c = proof_old_cv.C + bal_proof.old_bal_com + (proof.old_com_base * hash_pub_key_to_fr(&proof.wpk));
//        let new_proof_old_cv = clproto::ProofCV { T: proof_old_cv.T,
//                                         C: new_c,
//                                         s: proof_old_cv.s.clone(),
//                                         pub_bases: proof_old_cv.pub_bases.clone(),
//                                         num_secrets: proof_old_cv.num_secrets };
//        let is_wpk_valid_reveal = clproto::bs_verify_nizk_proof(&new_proof_old_cv);
//        if !is_wpk_valid_reveal {
//            panic!("pay_by_merchant_phase1 - failed to verify NIZK PoK of committed values that reveals wpk!");
//        }

        let is_existing_wpk = exist_in_merchant_state(&state, &proof.wpk, None);
        let bal_inc_within_range = bal_proof.balance_increment >= -E_MAX && bal_proof.balance_increment <= E_MAX;
        // check the range proof of the updated balance
        let mut transcript = Transcript::new(b"BOLT Range Proof");
        let pc_gens = PedersenGens::default();
        let is_range_proof_valid = proof.proof3.range_proof.0.verify_single(&pp.bp_gens, &pc_gens,
                                                                   &mut transcript, &proof.proof3.range_proof.1,
                                                                   pp.range_proof_bits).is_ok();

        // if above is is_wpk_valid_reveal => true, then we can proceed to
        // check that the proof of valid signature and then
        if proof_vs_old_wallet && !is_existing_wpk && bal_inc_within_range && is_range_proof_valid {
            println!("Proof of knowledge of signature is valid!");
            if bal_proof.balance_increment < 0 {
                // negative increment
                state.R = 1;
            } else {
                // postiive increment
                state.R = -1; // -1 denotes \bot here
            }
        } else {
            panic!("pay_by_merchant_phase1 - Verification failure for old wallet signature contents!");
        }

        // now we can verify the proof of knowledge for committed values in new wallet
        if !proof.bal_proof.third_party {
            let bal_inc_fr = -convert_int_to_fr(bal_proof.balance_increment);
            // check that the PoK on new wallet commitment is valid and
            // the updated balance differs by the balance increment from the balance
            // in previous wallet
            let bal_index = 2;
            let w_com_pr = bal_proof.w_com_pr_pr + bal_proof.old_bal_com + (proof_cv.pub_bases[bal_index] * bal_inc_fr);
            if proof_cv.C != w_com_pr {
                panic!("pay_by_merchant_phase1 - Old and new balance does not differ by payment amount!");
            }
        } else {
            // in third party case, what we do a PoK for committed payment increment
            let proof_vcom = proof.bal_proof.proof_vcom.as_ref().unwrap();
            if !clproto::bs_verify_nizk_proof(&proof_vcom) {
                panic!("pay_by_merchant_phase1 - Could not verify the NIZK PoK of payment amount");
            }
        }

        if clproto::bs_verify_nizk_proof(&proof_cv) {
            // generate refund token on new wallet
            let i = pk_m.Z2.len()-1;
            let c_refund = proof_cv.C + (pk_m.Z2[i] * convert_str_to_fr("refund"));
            // generating partially blind signature on refund || wpk' || B - e
            let rt_w = clproto::bs_compute_blind_signature(&pp.cl_mpk, &sk_m, c_refund, proof_cv.num_secrets + 1);
            println!("pay_by_merchant_phase1 - Proof of knowledge of commitment on new wallet is valid");
            update_merchant_state(&mut state, &proof.wpk, None);
            state.pay_init = true;
            return rt_w;
        }

        panic!("pay_by_merchant_phase1 - NIZK verification failed for new wallet commitment!");
    }

    ///
    /// Verify third party payment proof from two bi-directional channel payments with intermediary
    ///
    pub fn verify_third_party_payment(pp: &PublicParams, fee: i32, proof1: &BalanceProof, proof2: &BalanceProof) -> bool {
        if proof1.third_party && proof2.third_party {
            let vcom1 = &proof1.proof_vcom.as_ref().unwrap();
            let vcom2 = &proof2.proof_vcom.as_ref().unwrap();
            let rproof1 = &proof1.proof_vrange.as_ref().unwrap();
            let rproof2 = &proof2.proof_vrange.as_ref().unwrap();
            let pc_gens1 = PedersenGens::default();
            let pc_gens2 = PedersenGens::default();
            let mut transcript1 = Transcript::new(b"Range Proof for Balance Increment");
            let range_proof1_valid = rproof1.range_proof.0.verify_single(&pp.bp_gens, &pc_gens1,
                                                                  &mut transcript1,
                                                                  &rproof1.range_proof.1,
                                                                  pp.range_proof_bits).is_ok();

            let mut transcript2 = Transcript::new(b"Range Proof for Balance Increment");
            let range_proof2_valid = rproof2.range_proof.0.verify_single(&pp.bp_gens, &pc_gens2,
                                                                 &mut transcript2,
                                                                 &rproof2.range_proof.1,
                                                                 pp.range_proof_bits).is_ok();

            let len = vcom1.pub_bases.len();
            assert!(len >= 2 && vcom1.pub_bases.len() == vcom2.pub_bases.len());

            // g^(e1 + -e2 + fee) * h^(r1 + r2) ==> should be equal to g^(fee) * h^(r1 + r2)
            // lets add commitments for vcom1 and vcom2 to check
            let added_commits = vcom1.C + vcom2.C;
            let tx_fee = vcom1.pub_bases[1] * -convert_int_to_fr(fee);
            // compute h^r1 + r2
            let h_r1_r2 = (vcom1.pub_bases[0] * proof1.vcom.unwrap().r) +
                (vcom2.pub_bases[0] * proof2.vcom.unwrap().r) + tx_fee;

            let is_pay_plus_fee = added_commits == h_r1_r2;
            return clproto::bs_verify_nizk_proof(&vcom1) &&
                clproto::bs_verify_nizk_proof(&vcom2) &&
                range_proof1_valid && range_proof2_valid &&
                is_pay_plus_fee;
        }
        panic!("verify_third_party_payment - third-party payment not enabled for both proofs");
    }


    ///
    /// pay_by_customer_phase2 - takes as input the public params, old wallet, new wallet,
    /// merchant's verification key and refund token. If the refund token is valid, generate
    /// a revocation token for the old wallet public key.
    ///
    pub fn pay_by_customer_phase2(pp: &PublicParams, old_w: &CustomerWallet, new_w: &CustomerWallet,
                                  pk_m: &clsigs::PublicKeyD, rt_w: &clsigs::SignatureD) -> RevokeToken {
        // (1) verify the refund token (rt_w) against the new wallet contents
        let bal = convert_int_to_fr(new_w.balance);
        let h_wpk = hash_pub_key_to_fr(&new_w.wpk);
        let refund = convert_str_to_fr("refund");
        let mut x: Vec<Fr> = vec![new_w.r.clone(), new_w.cid.clone(), bal, h_wpk, refund];

        let is_rt_w_valid = clsigs::verify_d(&pp.cl_mpk, &pk_m, &x, &rt_w);
        if is_rt_w_valid {
            println!("Refund token is valid against the new wallet!");
            let schnorr = secp256k1::Secp256k1::new();
            let rm = RevokedMessage::new(String::from("revoked"), old_w.wpk, None);
            let msg = secp256k1::Message::from_slice(&rm.hash_to_slice()).unwrap();
            // msg = "revoked"|| old_wpk (for old wallet)
            let rv_w = schnorr.sign(&msg, &old_w.wsk);
            // return the revocation token
            return RevokeToken { message: rm, signature: rv_w };
        }
        panic!("pay_by_customer_phase2 - Merchant did not provide a valid refund token!");
    }

    ///
    /// pay_by_merchant_phase2 - takes as input the public params, channel state, proof of payment,
    /// merchant wallet, and revocation token from the customer. If the revocation token is valid,
    /// generate a new signature for the new wallet (from the PoK of committed values in new wallet).
    ///
    pub fn pay_by_merchant_phase2(pp: &PublicParams, mut state: &mut ChannelState,
                                  proof: &PaymentProof, m_data: &mut InitMerchantData,
                                  rv: &RevokeToken) -> clsigs::SignatureD {
        let proof_cv = &proof.proof2a;
        let sk_m = &m_data.csk.sk;
        let schnorr = secp256k1::Secp256k1::new();
        let msg = secp256k1::Message::from_slice(&rv.message.hash_to_slice()).unwrap();
        // verify that the revocation token is valid
        let is_rv_valid = schnorr.verify(&msg, &rv.signature, &proof.wpk).is_ok();

        if clproto::bs_verify_nizk_proof(&proof_cv) && is_rv_valid {
            // update merchant state with (wpk, sigma_rev)
            update_merchant_state(&mut state, &proof.wpk, Some(rv.signature));
            let new_wallet_sig = clproto::bs_compute_blind_signature(&pp.cl_mpk, &sk_m, proof_cv.C, proof_cv.num_secrets);
            m_data.csk.balance += proof.bal_proof.balance_increment + state.tx_fee;
            state.R = 2;
            return new_wallet_sig;
        }

        panic!("pay_by_merchant_phase2 - Customer did not provide valid revocation token!");
    }

    ///
    /// pay_by_customer_final - takes as input the public params, merchant's verification key,
    /// customer's old wallet, new channel token, new wallet and wallet signature (from merchant).
    /// Update the new wallet accordingly and checks if the signature from merchant is valid.
    ///
    pub fn pay_by_customer_final(pp: &PublicParams, pk_m: &clsigs::PublicKeyD,
                                     c_data: &mut InitCustomerData, mut new_t: ChannelToken,
                                     mut new_w: CustomerWallet, sig: clsigs::SignatureD) -> bool {
        if new_w.signature.is_none() {
            if pp.extra_verify {
                let bal = convert_int_to_fr(new_w.balance);
                let h_wpk = hash_pub_key_to_fr(&new_w.wpk);
                let mut x: Vec<Fr> = vec![new_w.r.clone(), new_w.cid.clone(), bal, h_wpk];
                assert!(clsigs::verify_d(&pp.cl_mpk, &pk_m, &x, &sig));
            }
            // update signature in new wallet
            new_w.signature = Some(sig);
            // update csk in new wallet
            c_data.csk = new_w;
            // update the channel token
            c_data.channel_token = new_t;
            return true;
        }
        // must be an old wallet
        return false;
    }

    ///// end of pay protocol

    // for customer => on input a wallet w, it outputs a customer channel closure message rc_c
    ///
    /// customer_refund - takes as input the public params, channel state, merchant's verification
    /// key, and customer wallet. Generates a channel closure message for customer.
    ///
    pub fn customer_refund(pp: &PublicParams, state: &ChannelState, pk_m: &clsigs::PublicKeyD,
                           w: &CustomerWallet) -> ChannelclosureC {
        let m;
        let balance = w.balance as usize;
        if !state.pay_init {
            // pay protocol not invoked so take the balance
            m = RefundMessage::new(String::from("refundUnsigned"), w.wpk, balance, Some(w.r), None);
        } else {
            // if channel has already been activated, then take unspent funds
            m = RefundMessage::new(String::from("refundToken"), w.wpk, balance, None, w.refund_token.clone());
        }

        // generate signature on the balance/channel id, etc to obtain funds back
        let m_vec = m.hash();
        let sigma = clsigs::sign_d(&pp.cl_mpk, &w.sk, &m_vec);
        return ChannelclosureC { message: m, signature: sigma };
    }

    fn exist_in_merchant_state(state: &ChannelState, wpk: &secp256k1::PublicKey, rev: Option<secp256k1::Signature>) -> bool {
        if state.keys.is_empty() {
            return false;
        }

        let fingerprint = compute_pub_key_fingerprint(wpk);
        if state.keys.contains_key(&fingerprint) {
            let pub_key = state.keys.get(&fingerprint).unwrap();
            if pub_key.revoke_token.is_none() {
                // let's just check the public key
                return pub_key.wpk == *wpk;
            }
            if !rev.is_none() {
                return pub_key.wpk == *wpk && pub_key.revoke_token.unwrap() == rev.unwrap();
            }
            return pub_key.wpk == *wpk;
        }

        return false;
    }

    fn update_merchant_state(state: &mut ChannelState, wpk: &secp256k1::PublicKey, rev: Option<secp256k1::Signature>) -> bool {
        let fingerprint = compute_pub_key_fingerprint(wpk);
        //println!("Print fingerprint: {}", fingerprint);
        if !rev.is_none() {
            let cust_pub_key = PubKeyMap { wpk: wpk.clone(), revoke_token: Some(rev.unwrap().clone()) };
            state.keys.insert(fingerprint, cust_pub_key);
        } else {
            let cust_pub_key = PubKeyMap { wpk: wpk.clone(), revoke_token: None };
            state.keys.insert(fingerprint, cust_pub_key);
        }
        return true;
    }

    ///
    /// merchant_refute - takes as input the public params, channel token, merchant's wallet,
    /// channels tate, channel closure from customer, and revocation token.
    /// Generates a channel closure message for merchant and updated merchant internal state.
    ///
    pub fn merchant_refute(pp: &PublicParams, state: &mut ChannelState, t_c: &ChannelToken, m_data: &InitMerchantData,
                           rc_c: &ChannelclosureC, rv_token: &secp256k1::Signature)  -> ChannelclosureM {
        // for merchant => on input the merchant's current state S_old and a customer channel closure message,
        // outputs a merchant channel closure message rc_m and updated merchant state S_new
        let is_valid = clsigs::verify_d(&pp.cl_mpk, &t_c.pk, &rc_c.message.hash(), &rc_c.signature);
        if is_valid {
            let wpk = rc_c.message.wpk;
            let balance = rc_c.message.balance;
            if !exist_in_merchant_state(&state, &wpk, Some(*rv_token)) {
                // update state to include the user's wallet key
                assert!(update_merchant_state(state, &wpk, Some(*rv_token)));
            }
            let ser_rv_token = rv_token.serialize_compact();
            let rm = RevokedMessage::new(String::from("revoked"), wpk, Some(ser_rv_token));
            // sign the revoked message
            let signature = clsigs::sign_d(&pp.cl_mpk, &m_data.csk.sk, &rm.hash());
            return ChannelclosureM { message: rm, signature: signature };
        } else {
            panic!("Signature on customer closure message is invalid!");
        }
    }

    ///
    /// resolve - on input the customer and merchant channel tokens T_c, T_m, along with
    /// closure messages rc_c, rc_m.
    /// this will be executed by the network to make sure the right balance is returned
    /// to each party based on provided inputs.
    ///
    pub fn resolve(pp: &PublicParams, c: &InitCustomerData, m: &InitMerchantData,
                   rc_c: Option<ChannelclosureC>, rc_m: Option<ChannelclosureM>,
                   rt_w: Option<clsigs::SignatureD>) -> (i32, i32) {
        let total_balance = c.csk.balance + m.csk.balance;
        if rc_c.is_none() && rc_m.is_none() {
            panic!("resolve1 - Did not specify channel closure messages for either customer or merchant!");
        }

        if rc_c.is_none() {
            // could not find customer's channel closure message.
            // judgement: give merchant everything
            return (0, total_balance);
        }

        let pk_c = &c.channel_token.pk; // get public key for customer
        let pk_m = &m.channel_token; // get public key for merchant

        let rc_cust = rc_c.unwrap();
        let rcc_valid = clsigs::verify_d(&pp.cl_mpk, &pk_c, &rc_cust.message.hash(), &rc_cust.signature);
        if !rcc_valid {
            panic!("resolve2 - rc_c signature is invalid!");
        }
        let msg = &rc_cust.message;
        let w_com = &c.channel_token.w_com;

        if msg.msgtype == "refundUnsigned" {
            // assert the validity of the w_com
            let cm_csp = generate_commit_setup(&pp, &pk_m);

            let h_wpk = hash_pub_key_to_fr(&c.csk.wpk);
            // convert balance into Fr
            let balance = convert_int_to_fr(c.csk.balance);
            let mut x: Vec<Fr> = vec![w_com.r, c.csk.cid, h_wpk, balance];

            // check that w_com is a valid commitment
            if !commit_scheme::decommit(&cm_csp, &w_com, &x) {
                // if this fails, then customer gets 0 and merchant gets full channel balance
                println!("resolve3 - failed verify commitment on wallet");
                return (0, total_balance);
            }
        } else if msg.msgtype == "refundToken" {
            // check that the refund token for specified wallet is valid
            let bal = convert_int_to_fr(msg.balance as i32);
            let h_wpk = hash_pub_key_to_fr(&msg.wpk);
            let refund = convert_str_to_fr("refund");
            let mut x: Vec<Fr> = vec![c.csk.r.clone(), c.csk.cid.clone(), bal, h_wpk, refund];

            let is_rt_valid = clsigs::verify_d(&pp.cl_mpk, &pk_m, &x, &rt_w.unwrap());
            if !is_rt_valid {
                // refund token signature not valid, so pay full channel balance to merchant
                return (0, total_balance)
            }
        }


        if !rc_m.is_none() {
            let rc_merch = rc_m.unwrap();
            let refute_valid = clsigs::verify_d(&pp.cl_mpk, &pk_m, &rc_merch.message.hash(), &rc_merch.signature);
            if !refute_valid {
                // refute token is invalid, so return customer balance and merchant balance
                return (c.csk.balance, m.csk.balance);
            } else {
                // if refutation is valid
                return (0, total_balance);
            }
        }

        panic!("resolve4 - Did not specify channel closure messages for either customer or merchant!");
    }
}

#[no_mangle]
pub mod ffishim {
    extern crate libc;

    use bidirectional;
    use clsigs;
    use commit_scheme;
    use clproto;

    use serde::{Serialize, Deserialize};

    use libc::{c_char};
    use std::ffi::{CStr, CString};
    use std::str;
    use std::mem;

    use bn::Fr;

    use serialization_wrappers;

    fn deserialize_object<'a, T>(serialized: *mut c_char) -> T 
	where
	    T: Deserialize<'a>,
	{  // TODO make this a result with nice error handling
	    let bytes = unsafe { CStr::from_ptr(serialized).to_bytes() };
	    let string: &str = str::from_utf8(bytes).unwrap(); // make sure the bytes are UTF-8
	    serde_json::from_str(&string).unwrap()
	}

    fn deserialize_optional_object<'a, T>(serialized: *mut c_char) -> Option<T> 
    where
        T: Deserialize<'a>,
    {  // TODO make this a result with nice error handling
        let bytes = unsafe { CStr::from_ptr(serialized).to_bytes() };
        let string: &str = str::from_utf8(bytes).unwrap(); // make sure the bytes are UTF-8
        Some(serde_json::from_str(&string).unwrap())
    }

    #[no_mangle]
    pub extern fn ffishim_free_string(pointer: *mut c_char) {
        unsafe{ 
            if pointer.is_null() { return }
            CString::from_raw(pointer) 
        };
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_setup(extra_verify: u32) -> *mut c_char {
        let mut ev = false;
        if extra_verify > 1 {
            ev = true;
        }
        let pp = bidirectional::setup(ev);
        let ser = ["{\'pp\':\'",serde_json::to_string(&pp).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_channelstate_new(channel_name: *const c_char, third_party_support: u32) -> *mut c_char {
    
        let bytes = unsafe { CStr::from_ptr(channel_name).to_bytes() };
        let name: &str = str::from_utf8(bytes).unwrap(); // make sure the bytes are UTF-8

        let mut tps = false;
        if third_party_support > 1 {
            tps = true;
        }
        let channel = bidirectional::ChannelState::new(name.to_string(), tps);
        let ser = ["{\'state\':\'",serde_json::to_string(&channel).unwrap().as_str(), "\'}"].concat();;
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_keygen(serialized_pp: *mut c_char) -> *mut c_char {

        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        let keypair = bidirectional::keygen(&deserialized_pp);
        let ser = ["{\'keypair\':\'",serde_json::to_string(&keypair).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }


    #[no_mangle]
    pub extern fn ffishim_bidirectional_init_merchant(serialized_pp: *mut c_char, balance_merchant: i32, serialized_merchant_keypair: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the merchant keypair 
        let deserialized_merchant_keypair: clsigs::KeyPairD = deserialize_object(serialized_merchant_keypair);

        let init_merchant_data = bidirectional::init_merchant(&deserialized_pp, balance_merchant, &deserialized_merchant_keypair);
        let ser = ["{\'merchant_data\':\'", serde_json::to_string(&init_merchant_data).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_generate_commit_setup(serialized_pp: *mut c_char, serialized_merchant_public_key: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        let cm_csp = bidirectional::generate_commit_setup(&deserialized_pp, &deserialized_merchant_public_key);
        let ser = ["{\'commit_setup\':\'", serde_json::to_string(&cm_csp).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_init_customer(serialized_pp: *mut c_char, serialized_channel: *mut c_char, balance_customer: i32,  balance_merchant: i32, serialized_commitment_setup: *mut c_char, serialized_customer_keypair: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let mut deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel);

        // Deserialize the commitment setup
        let deserialized_ccommitment_setup: commit_scheme::CSParams = deserialize_object(serialized_commitment_setup); 

        // Deserialize the client keypair 
        let deserialized_customer_keypair: clsigs::KeyPairD = deserialize_object(serialized_customer_keypair);

        // We change the channel state
        let cust_data = bidirectional::init_customer(&deserialized_pp, &mut deserialized_channel_state, balance_customer, balance_merchant, &deserialized_ccommitment_setup, &deserialized_customer_keypair);
        let ser = ["{\'customer_data\':\'", serde_json::to_string(&cust_data).unwrap().as_str(), "\', \'state\':\'", serde_json::to_string(&deserialized_channel_state).unwrap().as_str() ,"\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_establish_customer_phase1(serialized_pp: *mut c_char, serialized_customer_data: *mut c_char, serialized_merchant_data: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the custdata
        let deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data); 

        // Deserialize the merchant data
        let deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data); 

        let proof1 = bidirectional::establish_customer_phase1(&deserialized_pp, &deserialized_customer_data, &deserialized_merchant_data.bases);
        let ser = ["{\'proof\':\'", serde_json::to_string(&proof1).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_establish_merchant_phase2(serialized_pp: *mut c_char, serialized_channel: *mut c_char, serialized_merchant_data: *mut c_char, serialized_proof1: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let mut deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the merchant data
        let deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data);

        // Deserialize the first proof
        let deserialized_proof_1: clproto::ProofCV = deserialize_object(serialized_proof1); 

        let wallet_sig = bidirectional::establish_merchant_phase2(&deserialized_pp, &mut deserialized_channel_state, &deserialized_merchant_data, &deserialized_proof_1);
        let ser = ["{\'wallet_sig\':\'", serde_json::to_string(&wallet_sig).unwrap().as_str(), "\', \'state\':\'", serde_json::to_string(&deserialized_channel_state).unwrap().as_str() ,"\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_bidirectional_establish_customer_final(serialized_pp: *mut c_char, serialized_merchant_public_key: *mut c_char, serialized_customer_data: *mut c_char, serialized_wallet_sig: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        // Deserialize the custdata
        let mut deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data); 

        // Deserialize the wallet_sig
        let deserialized_wallet_sig: clsigs::SignatureD = deserialize_object(serialized_wallet_sig); 

        bidirectional::establish_customer_final(&deserialized_pp, &deserialized_merchant_public_key, &mut deserialized_customer_data.csk, deserialized_wallet_sig);
        let ser = ["{\'customer_data\':\'", serde_json::to_string(&deserialized_customer_data).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_customer_phase1_precompute(serialized_pp: *mut c_char,  serialized_customer_data: *mut c_char, serialized_merchant_public_key: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the custdata
        let mut deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data); 

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        bidirectional::pay_by_customer_phase1_precompute(&deserialized_pp, &deserialized_customer_data.channel_token, &deserialized_merchant_public_key, &mut deserialized_customer_data.csk);
        let ser = ["{\'customer_data\':\'", serde_json::to_string(&deserialized_customer_data).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_customer_phase1(serialized_pp: *mut c_char, serialized_channel: *mut c_char, serialized_customer_data: *mut c_char, serialized_merchant_public_key: *mut c_char, balance_increment: i32) -> *mut c_char  {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the custdata
        let deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        let (t_c, new_wallet, pay_proof) = bidirectional::pay_by_customer_phase1(&deserialized_pp, &deserialized_channel_state, &deserialized_customer_data.channel_token,  &deserialized_merchant_public_key,  &deserialized_customer_data.csk, balance_increment);
        let ser = ["{\'channel_token\':\'", serde_json::to_string(&t_c).unwrap().as_str(), "\', \'new_wallet\':\'", serde_json::to_string(&new_wallet).unwrap().as_str() ,  "\', \'pay_proof\':\'", serde_json::to_string(&pay_proof).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_merchant_phase1(serialized_pp: *mut c_char, serialized_channel: /*make mut*/ *mut c_char, serialized_pay_proof: *mut c_char, serialized_merchant_data: *mut c_char) -> *mut c_char  {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let mut deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the pay proof
        let deserialized_pay_proof: bidirectional::PaymentProof = deserialize_object(serialized_pay_proof); 

        // Deserialize the merchant data
        let deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data);

        let rt_w = bidirectional::pay_by_merchant_phase1(&deserialized_pp, &mut deserialized_channel_state, &deserialized_pay_proof, &deserialized_merchant_data);
        let ser = ["{\'rt_w\':\'", serde_json::to_string(&rt_w).unwrap().as_str(), "\', \'state\':\'", serde_json::to_string(&deserialized_channel_state).unwrap().as_str() ,"\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_customer_phase2(serialized_pp: *mut c_char, serialized_customer_data: *mut c_char, serialized_new_wallet: *mut c_char, serialized_merchant_public_key: *mut c_char, serialized_rt_w: *mut c_char) -> *mut c_char  {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the custdata
        let deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data);

        // Deserialize the new wallet
        let deserialized_new_wallet: bidirectional::CustomerWallet = deserialize_object(serialized_new_wallet); 

        // Deserialize the merchant public key
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        // Deserialize the rt_w
        let deserialized_rt_w: clsigs::SignatureD = deserialize_object(serialized_rt_w); 

        // RevokeToken
        let rv_w = bidirectional::pay_by_customer_phase2(&deserialized_pp, &deserialized_customer_data.csk, &deserialized_new_wallet, &deserialized_merchant_public_key, &deserialized_rt_w);
        let ser = ["{\'rv_w\':\'", serde_json::to_string(&rv_w).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_merchant_phase2(serialized_pp: *mut c_char, serialized_channel: /*make mut*/ *mut c_char, serialized_pay_proof: *mut c_char, serialized_merchant_data:  /*make mut*/ *mut c_char, serialized_revoke_token: *mut c_char ) -> *mut c_char  {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let mut deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the pay proof
        let deserialized_pay_proof: bidirectional::PaymentProof = deserialize_object(serialized_pay_proof); 

        // Deserialize the merchant data
        let mut deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data);

        // Deserialize the merchant revoke token
        let deserialized_revoke_token: bidirectional::RevokeToken = deserialize_object(serialized_revoke_token);

        let new_wallet_sig = bidirectional::pay_by_merchant_phase2(&deserialized_pp, &mut deserialized_channel_state, &deserialized_pay_proof, &mut deserialized_merchant_data, &deserialized_revoke_token);
        let ser = ["{\'new_wallet_sig\':\'", serde_json::to_string(&new_wallet_sig).unwrap().as_str(), "\', \'state\':\'", serde_json::to_string(&deserialized_channel_state).unwrap().as_str() ,  "\', \'merch_data\':\'", serde_json::to_string(&deserialized_merchant_data).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_pay_by_customer_final(serialized_pp: *mut c_char, serialized_merchant_public_key: *mut c_char, serialized_customer_data: /* make mut */ *mut c_char, serialized_channel_token: *mut c_char, serialized_new_wallet: *mut c_char, serialized_new_wallet_sig: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        // Deserialize the custdata
        let mut deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data); 

        // Deserialize the channel token
        let deserialized_channel_token: bidirectional::ChannelToken = deserialize_object(serialized_channel_token);

        // Deserialize the new wallet
        let deserialized_new_wallet: bidirectional::CustomerWallet = deserialize_object(serialized_new_wallet); 

        // Deserialize the new wallet sig
        let deserialized_new_wallet_sig: clsigs::SignatureD = deserialize_object(serialized_new_wallet_sig); 

        bidirectional::pay_by_customer_final(&deserialized_pp, &deserialized_merchant_public_key, &mut deserialized_customer_data, deserialized_channel_token, deserialized_new_wallet, deserialized_new_wallet_sig);
        let ser = ["{\'customer_data\':\'", serde_json::to_string(&deserialized_customer_data).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_customer_refund(serialized_pp: *mut c_char, serialized_channel: *mut c_char, serialized_merchant_public_key: *mut c_char,  serialized_wallet: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        // Deserialize the new wallet
        let deserialized_wallet: bidirectional::CustomerWallet = deserialize_object(serialized_wallet); 

        let rc_c = bidirectional::customer_refund(&deserialized_pp, &deserialized_channel_state, &deserialized_merchant_public_key, &deserialized_wallet);
        let ser = ["{\'rc_c\':\'", serde_json::to_string(&rc_c).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }


    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_merchant_refund(serialized_pp: *mut c_char, serialized_channel: *mut c_char, serialized_channel_token: *mut c_char, serialized_merchant_data: *mut c_char,  serialized_channel_closure: *mut c_char, serialized_revoke_token: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the channel state
        let mut deserialized_channel_state: bidirectional::ChannelState = deserialize_object(serialized_channel); 

        // Deserialize the channel token
        let deserialized_channel_token: bidirectional::ChannelToken = deserialize_object(serialized_channel_token);

        // Deserialize the merchant data
        let deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data); 

        // Deserialize the closure
        let deserialized_channel_closure: bidirectional::ChannelclosureC = deserialize_object(serialized_channel_closure);

        // Deserialize the revoke_token
        let deserialized_revoke_token: secp256k1::Signature = deserialize_object(serialized_revoke_token);
        
        let rc_m = bidirectional::merchant_refute(&deserialized_pp, &mut deserialized_channel_state, &deserialized_channel_token, &deserialized_merchant_data, &deserialized_channel_closure, &deserialized_revoke_token);
        let ser = ["{\'rc_m\':\'", serde_json::to_string(&rc_m).unwrap().as_str(), "\', \'state\':\'", serde_json::to_string(&deserialized_channel_state).unwrap().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }
 
    #[no_mangle]                        
    pub extern fn ffishim_bidirectional_resolve(serialized_pp: *mut c_char, serialized_customer_data: *mut c_char, serialized_merchant_data: *mut c_char, serialized_closure_customer: *mut c_char,  serialized_closure_merchant: *mut c_char, serialized_revoke_token: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the custdata
        let deserialized_customer_data: bidirectional::InitCustomerData = deserialize_object(serialized_customer_data);

        // Deserialize the merchant data
        let deserialized_merchant_data: bidirectional::InitMerchantData = deserialize_object(serialized_merchant_data); 

        //TODO handle none()

        // Deserialize the client closure
        let deserialized_closure_customer: bidirectional::ChannelclosureC = deserialize_object(serialized_closure_customer);

        // Deserialize the merchant closure
        let deserialized_closure_merchant: bidirectional::ChannelclosureM = deserialize_object(serialized_closure_merchant);

        // Deserialize the revoke_token
        let deserialized_revoke_token: clsigs::SignatureD = deserialize_object(serialized_revoke_token);

        let (new_b0_cust, new_b0_merch) = bidirectional::resolve(&deserialized_pp, &deserialized_customer_data, &deserialized_merchant_data, Some(deserialized_closure_customer), Some(deserialized_closure_merchant), Some(deserialized_revoke_token));
        let ser = ["{\'new_b0_cust\':\'", new_b0_cust.to_string().as_str(), "\', \'new_b0_merch\':\'", new_b0_merch.to_string().as_str(), "\'}"].concat();
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]                        
    pub extern fn ffishim_commit_scheme_decommit(serialized_csp: *mut c_char, serialized_commitment: *mut c_char, serialized_x: *mut c_char) -> *mut c_char {
        // Deserialize the csp
        let deserialized_csp: commit_scheme::CSParams = deserialize_object(serialized_csp);

        // Deserialize the commit
        let deserialized_commitment: commit_scheme::Commitment = deserialize_object(serialized_commitment);

        // Deserialize the vec<fr> x
        let deserialized_x: serialization_wrappers::VecFrWrapper = deserialize_object(serialized_x);
            // Wrapper struct is required because Serde needs something to annotate

        let ser = match commit_scheme::decommit(&deserialized_csp, &deserialized_commitment, &deserialized_x.0) {
            false => "{\'return_value\':\'false\'}",
            true => "{\'return_value\':\'true\'}",
        };
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_validate_channel_open(serialized_channel_token: *mut c_char, serialized_messages: *mut c_char) -> *mut c_char {

        // Deserialize the channel token
        let deserialized_channel_token: serialization_wrappers::WalletCommitmentAndParamsWrapper = deserialize_object(serialized_channel_token);

        // Deserialize the vec<fr> x
        let deserialized_messages: serialization_wrappers::VecFrWrapper = deserialize_object(serialized_messages);

        let ser = match commit_scheme::decommit(&deserialized_channel_token.params, &deserialized_channel_token.com, &deserialized_messages.0) {
            false => "{\'return_value\':\'false\'}",
            true => "{\'return_value\':\'true\'}",
        };
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_validate_channel_close(serialized_pp: *mut c_char, serialized_closure_customer: *mut c_char, serialized_merchant_public_key: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the customer closure
        let deserialized_closure_customer: bidirectional::ChannelclosureC = deserialize_object(serialized_closure_customer);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        //validate signature 
        let ser = match clsigs::verify_d(&deserialized_pp.cl_mpk, &deserialized_merchant_public_key, &deserialized_closure_customer.message.hash(), &deserialized_closure_customer.signature) {
            false => "{\'return_value\':\'false\'}",
            true => "{\'return_value\':\'true\'}",
        };
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }

    #[no_mangle]
    pub extern fn ffishim_resolve_channel_dispute(serialized_pp: *mut c_char, serialized_channel_closure_message_customer: *mut c_char, serialized_channel_token_client: *mut c_char, serialized_channel_closure_message_merchant: *mut c_char, serialized_merchant_public_key: *mut c_char) -> *mut c_char {
        // Deserialize the pp
        let deserialized_pp: bidirectional::PublicParams = deserialize_object(serialized_pp);

        // Deserialize the customer closure
        let deseralized_customer_closure: bidirectional::ChannelclosureC = deserialize_object(serialized_channel_closure_message_customer);

        // Deserialize the Channel Token
        let deserialized_channel_token: bidirectional::ChannelToken = deserialize_object(serialized_channel_token_client); 
 
        // Deserialize the merchant closure
        let deserialized_closure_merchant: bidirectional::ChannelclosureM = deserialize_object(serialized_channel_closure_message_merchant);

        // Deserialize the merchant keypair 
        let deserialized_merchant_public_key: clsigs::PublicKeyD = deserialize_object(serialized_merchant_public_key);

        //Verify the revocation token 
        let ser = match clsigs::verify_d(&deserialized_pp.cl_mpk, &deserialized_merchant_public_key, &deserialized_closure_merchant.message.hash(), &deserialized_closure_merchant.signature) {
            false => "{\'return_value\':\'false\'}",
            true => "{\'return_value\':\'true\'}",     
        };
        let cser = CString::new(ser).unwrap();
        cser.into_raw()
    }
}

#[cfg(all(test, feature = "unstable"))]
mod benches {
    use rand::{Rng, thread_rng};
    use test::{Bencher, black_box};

    #[bench]
    pub fn bench_one(bh: &mut Bencher) {
        println!("Run benchmark tests here!");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn unidirectional_payment_basics_work() {
        // TODO: finish me
        assert!(true);
    }

    fn setup_new_channel_helper(pp: &bidirectional::PublicParams, channel: &mut bidirectional::ChannelState,
                                init_cust_bal: i32, init_merch_bal: i32)
                              -> (clsigs::KeyPairD, bidirectional::InitMerchantData,
                                  clsigs::KeyPairD, bidirectional::InitCustomerData) {

        let b0_cust = init_cust_bal;
        let b0_merch = init_merch_bal;

        // generate long-lived keypair for merchant -- used to identify
        // merchant to all customers
        let merch_keys = bidirectional::keygen(&pp);

        // customer generates an ephemeral keypair for use on a single channel
        let cust_keys = bidirectional::keygen(&pp);

        // each party executes the init algorithm on the agreed initial challenge balance
        // in order to derive the channel tokens
        // initialize on the merchant side with balance: b0_merch
        let merch_data = bidirectional::init_merchant(&pp, b0_merch, &merch_keys);

        // retrieve commitment setup params (using merchant long lived pk params)
        let cm_csp = bidirectional::generate_commit_setup(&pp, &merch_keys.pk);
        // initialize on the customer side with balance: b0_cust
        let cust_data = bidirectional::init_customer(&pp, channel,
                                                     b0_cust, b0_merch,
                                                     &cm_csp, &cust_keys);
        return (merch_keys, merch_data, cust_keys, cust_data);
    }

    fn setup_new_channel_existing_merchant_helper(pp: &bidirectional::PublicParams, channel: &mut bidirectional::ChannelState,
                                                 init_cust_bal: i32, init_merch_bal: i32, merch_keys: &clsigs::KeyPairD)
                                             -> (bidirectional::InitMerchantData, clsigs::KeyPairD, bidirectional::InitCustomerData) {

        let b0_cust = init_cust_bal;
        let b0_merch = init_merch_bal;

        // customer generates an ephemeral keypair for use on a single channel
        let cust_keys = bidirectional::keygen(&pp);

        // each party executes the init algorithm on the agreed initial challenge balance
        // in order to derive the channel tokens
        // initialize on the merchant side with balance: b0_merch
        let merch_data = bidirectional::init_merchant(&pp, b0_merch, &merch_keys);

        // retrieve commitment setup params (using merchant long lived pk params)
        let cm_csp = bidirectional::generate_commit_setup(&pp, &merch_keys.pk);
        // initialize on the customer side with balance: b0_cust
        let cust_data = bidirectional::init_customer(&pp, channel,
                                                     b0_cust, b0_merch,
                                                     &cm_csp, &cust_keys);
        return (merch_data, cust_keys, cust_data);
    }


    fn execute_establish_protocol_helper(pp: &bidirectional::PublicParams, channel: &mut bidirectional::ChannelState,
                                   merch_keys: &clsigs::KeyPairD, merch_data: &mut bidirectional::InitMerchantData,
                                   cust_keys: &clsigs::KeyPairD, cust_data: &mut bidirectional::InitCustomerData) {
        // entering the establish protocol for the channel
        let proof = bidirectional::establish_customer_phase1(&pp, &cust_data, &merch_data.bases);

        // obtain the wallet signature from the merchant
        let wallet_sig = bidirectional::establish_merchant_phase2(&pp, channel, &merch_data, &proof);

        // complete channel establishment
        assert!(bidirectional::establish_customer_final(&pp, &merch_keys.pk, &mut cust_data.csk, wallet_sig));
    }

    // pp, channel, merch_keys, merch_data, cust_keys, cust_data, pay_increment
    fn execute_pay_protocol_helper(pp: &bidirectional::PublicParams, channel: &mut bidirectional::ChannelState,
                                   merch_keys: &clsigs::KeyPairD, merch_data: &mut bidirectional::InitMerchantData,
                                   cust_keys: &clsigs::KeyPairD, cust_data: &mut bidirectional::InitCustomerData,
                                    payment_increment: i32) {
        // let's test the pay protocol
        bidirectional::pay_by_customer_phase1_precompute(&pp, &cust_data.channel_token, &merch_keys.pk, &mut cust_data.csk);

        let (t_c, new_wallet, pay_proof) = bidirectional::pay_by_customer_phase1(&pp, &channel, &cust_data.channel_token, // channel token
                                                                            &merch_keys.pk, // merchant pub key
                                                                            &cust_data.csk, // wallet
                                                                            payment_increment); // balance increment (FUNC INPUT)

        // get the refund token (rt_w)
        let rt_w = bidirectional::pay_by_merchant_phase1(&pp, channel, &pay_proof, &merch_data);

        // get the revocation token (rv_w) on the old public key (wpk)
        let rv_w = bidirectional::pay_by_customer_phase2(&pp, &cust_data.csk, &new_wallet, &merch_keys.pk, &rt_w);

        // get the new wallet sig (new_wallet_sig) on the new wallet
        let new_wallet_sig = bidirectional::pay_by_merchant_phase2(&pp, channel, &pay_proof, merch_data, &rv_w);

        assert!(bidirectional::pay_by_customer_final(&pp, &merch_keys.pk, cust_data, t_c, new_wallet, new_wallet_sig));
    }

    #[test]
    fn bidirectional_payment_basics_work() {
        let pp = bidirectional::setup(true);

        // just bidirectional case (w/o third party)
        let mut channel = bidirectional::ChannelState::new(String::from("Channel A -> B"), false);
        let total_owed = 40;
        let b0_customer = 90;
        let b0_merchant = 20;
        let payment_increment = 20;

        //let msg = "Open Channel ID: ";
        //libbolt::debug_elem_in_hex(msg, &channel.cid);

        let (merch_keys, mut merch_data, cust_keys, mut cust_data) = setup_new_channel_helper(&pp, &mut channel, b0_customer, b0_merchant);

        // run establish protocol for customer and merchant channel
        execute_establish_protocol_helper(&pp, &mut channel, &merch_keys, &mut merch_data, &cust_keys, &mut cust_data);

        assert!(channel.channel_established);

        {
            // make multiple payments in a loop
            let num_payments = total_owed / payment_increment;
            for i in 0 .. num_payments {
                execute_pay_protocol_helper(&pp, &mut channel, &merch_keys, &mut merch_data, &cust_keys, &mut cust_data, payment_increment);
            }

            {
                // scope localizes the immutable borrow here (for debug purposes only)
                let cust_wallet = &cust_data.csk;
                let merch_wallet = &merch_data.csk;
                println!("Customer balance: {}", cust_wallet.balance);
                println!("Merchant balance: {}", merch_wallet.balance);
                assert!(cust_wallet.balance == (b0_customer - total_owed) && merch_wallet.balance == total_owed + b0_merchant);
            }

            let cust_wallet = &cust_data.csk;
            // get channel closure message
            let rc_c = bidirectional::customer_refund(&pp, &channel, &merch_keys.pk, &cust_wallet);
            println!("Obtained the channel closure message: {}", rc_c.message.msgtype);
        }
    }

    #[test]
    fn bidirectional_payment_negative_payment_works() {
        let pp = bidirectional::setup(true);

        // just bidirectional case (w/o third party)
        let mut channel = bidirectional::ChannelState::new(String::from("Channel A <-> B"), false);
        let total_owed = -20;
        let b0_customer = 90;
        let b0_merchant = 30;
        let payment_increment = -20;

        //let msg = "Open Channel ID: ";
        //libbolt::debug_elem_in_hex(msg, &channel.cid);

        let (merch_keys, mut merch_data, cust_keys, mut cust_data) = setup_new_channel_helper(&pp, &mut channel, b0_customer, b0_merchant);

        // run establish protocol for customer and merchant channel
        execute_establish_protocol_helper(&pp, &mut channel, &merch_keys, &mut merch_data, &cust_keys, &mut cust_data);
        println!("Initial Customer balance: {}", cust_data.csk.balance);
        println!("Initial Merchant balance: {}", merch_data.csk.balance);

        assert!(channel.channel_established);

        {
            // make multiple payments in a loop
            execute_pay_protocol_helper(&pp, &mut channel, &merch_keys, &mut merch_data, &cust_keys, &mut cust_data, payment_increment);

            {
                // scope localizes the immutable borrow here (for debug purposes only)
                let cust_wallet = &cust_data.csk;
                let merch_wallet = &merch_data.csk;
                println!("Customer balance: {}", cust_wallet.balance);
                println!("Merchant balance: {}", merch_wallet.balance);
                assert!(cust_wallet.balance == (b0_customer - total_owed) && merch_wallet.balance == total_owed + b0_merchant);
            }
        }
    }

    fn execute_third_party_pay_protocol_helper(pp: &bidirectional::PublicParams,
                                   channel1: &mut bidirectional::ChannelState, channel2: &mut bidirectional::ChannelState,
                                   merch_keys: &clsigs::KeyPairD, merch1_data: &mut bidirectional::InitMerchantData,
                                   merch2_data: &mut bidirectional::InitMerchantData,
                                   cust1_keys: &clsigs::KeyPairD, cust1_data: &mut bidirectional::InitCustomerData,
                                   cust2_keys: &clsigs::KeyPairD, cust2_data: &mut bidirectional::InitCustomerData,
                                   payment_increment: i32) {
        // let's test the pay protocol
        bidirectional::pay_by_customer_phase1_precompute(&pp, &cust1_data.channel_token, &merch_keys.pk, &mut cust1_data.csk);
        bidirectional::pay_by_customer_phase1_precompute(&pp, &cust2_data.channel_token, &merch_keys.pk, &mut cust2_data.csk);

        println!("Channel 1 fee: {}", channel1.get_channel_fee());
        let (t_c1, new_wallet1, pay_proof1) = bidirectional::pay_by_customer_phase1(&pp, &channel1,
                                                                            &cust1_data.channel_token, // channel token
                                                                            &merch_keys.pk, // merchant pub key
                                                                            &cust1_data.csk, // wallet
                                                                            payment_increment); // balance increment
        println!("Channel 2 fee: {}", channel2.get_channel_fee());
        let (t_c2, new_wallet2, pay_proof2) = bidirectional::pay_by_customer_phase1(&pp, &channel2,
                                                                    &cust2_data.channel_token, // channel token
                                                                    &merch_keys.pk, // merchant pub key
                                                                    &cust2_data.csk, // wallet
                                                                    -payment_increment); // balance decrement

        // validate pay_proof1 and pay_proof2 (and the channel state for the fee paying channel, if fee > 0)
        let tx_fee = channel1.get_channel_fee() + channel2.get_channel_fee();
        assert!(bidirectional::verify_third_party_payment(&pp, tx_fee, &pay_proof1.bal_proof, &pay_proof2.bal_proof));

        // get the refund token (rt_w)
        let rt_w1 = bidirectional::pay_by_merchant_phase1(&pp, channel1, &pay_proof1, &merch1_data);

        // get the refund token (rt_w)
        let rt_w2 = bidirectional::pay_by_merchant_phase1(&pp, channel2, &pay_proof2, &merch2_data);

        // get the revocation token (rv_w) on the old public key (wpk)
        let rv_w1 = bidirectional::pay_by_customer_phase2(&pp, &cust1_data.csk, &new_wallet1, &merch_keys.pk, &rt_w1);

        // get the revocation token (rv_w) on the old public key (wpk)
        let rv_w2 = bidirectional::pay_by_customer_phase2(&pp, &cust2_data.csk, &new_wallet2, &merch_keys.pk, &rt_w2);

        // get the new wallet sig (new_wallet_sig) on the new wallet
        let new_wallet_sig1 = bidirectional::pay_by_merchant_phase2(&pp, channel1, &pay_proof1, merch1_data, &rv_w1);

        // get the new wallet sig (new_wallet_sig) on the new wallet
        let new_wallet_sig2 = bidirectional::pay_by_merchant_phase2(&pp, channel2, &pay_proof2, merch2_data, &rv_w2);

        assert!(bidirectional::pay_by_customer_final(&pp, &merch_keys.pk, cust1_data, t_c1, new_wallet1, new_wallet_sig1));

        assert!(bidirectional::pay_by_customer_final(&pp, &merch_keys.pk, cust2_data, t_c2, new_wallet2, new_wallet_sig2));
    }

    #[test]
    fn third_party_payment_basics_work() {
        let pp = bidirectional::setup(true);

        // third party -- so indicate so in the channel state
        let mut channel_a = bidirectional::ChannelState::new(String::from("Channel A <-> I"), true);
        let mut channel_b = bidirectional::ChannelState::new(String::from("Channel B <-> I"), true);

        let fee = 2;
        channel_a.set_channel_fee(fee);

        let total_payment = 20;
        let b0_alice = 30;
        let b0_bob = 30;
        let b0_merchant_a = 40;
        let b0_merchant_b = 40;

        let (merch_keys, mut merch_data_a, alice_keys, mut alice_data) = setup_new_channel_helper(&pp, &mut channel_a, b0_alice, b0_merchant_a);

        let (mut merch_data_b, bob_keys, mut bob_data) =
            setup_new_channel_existing_merchant_helper(&pp, &mut channel_b, b0_bob, b0_merchant_b, &merch_keys);

        // run establish protocol for alice and merchant channel
        execute_establish_protocol_helper(&pp, &mut channel_a, &merch_keys, &mut merch_data_a, &alice_keys, &mut alice_data);

        // run establish protocol for bob and merchant channel
        execute_establish_protocol_helper(&pp, &mut channel_b, &merch_keys, &mut merch_data_b, &bob_keys, &mut bob_data);

        assert!(channel_a.channel_established);
        assert!(channel_b.channel_established);

        // alice can pay bob through the merchant
        execute_third_party_pay_protocol_helper(&pp, &mut channel_a, &mut channel_b,
                                                &merch_keys, &mut merch_data_a, &mut merch_data_b,
                                                &alice_keys, &mut alice_data, &bob_keys, &mut bob_data, total_payment);

        println!("Customer alice balance: {}", alice_data.csk.balance);
        println!("Merchant channel balance with alice: {}", merch_data_a.csk.balance);
        println!("Customer bob balance: {}", bob_data.csk.balance);
        println!("Merchant channel balance with bob: {}", merch_data_b.csk.balance);
    }

    #[test]
    #[ignore]
    fn serialization_tests() {
        // TODO: finish me
        assert!(true);
    }

}
