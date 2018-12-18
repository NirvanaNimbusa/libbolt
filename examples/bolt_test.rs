extern crate bn;
extern crate rand;
extern crate rand_core;
extern crate bolt;
extern crate bincode;
extern crate time;
extern crate secp256k1;
//extern crate serde_derive;
//extern crate serde;

//use bolt::unidirectional;
use bolt::bidirectional;
use time::PreciseTime;

macro_rules! measure {
    ($x: expr) => {
        {
            let s = PreciseTime::now();
            let res = $x;
            let e = PreciseTime::now();
            (res, s.to(e))
        };
    }
}


macro_rules! measure_ret_mut {
    ($x: expr) => {
        {
            let s = PreciseTime::now();
            let mut handle = $x;
            let e = PreciseTime::now();
            (handle, s.to(e))
        };
    }
}

fn main() {
    println!("******************************************");
    // libbolt tests below
    println!("Testing the channel setup...");

    //println!("[1a] libbolt - setup bidirectional scheme params");
    let (pp, setup_time1) = measure!(bidirectional::setup(false));

    //println!("[1b] libbolt - generate the initial channel state");
    let mut channel = bidirectional::ChannelState::new(String::from("My New Channel A"), false);

    println!("Setup time: {}", setup_time1);

    //let msg = "Open Channel ID: ";
    //libbolt::debug_elem_in_hex(msg, &channel.cid);

    let b0_cust = 50;
    let b0_merch = 50;

    // generate long-lived keypair for merchant -- used to identify
    // it to all customers
    //println!("[2] libbolt - generate long-lived key pair for merchant");
    let (merch_keypair, _) = measure!(bidirectional::keygen(&pp));

    // customer generates an ephemeral keypair for use on a single channel
    println!("[3] libbolt - generate ephemeral key pair for customer (use with one channel)");
    let (cust_keypair, _) = measure!(bidirectional::keygen(&pp));

    // each party executes the init algorithm on the agreed initial challenge balance
    // in order to derive the channel tokens
    println!("[5a] libbolt - initialize on the merchant side with balance {}", b0_merch);
    let (mut merch_data, initm_time) = measure_ret_mut!(bidirectional::init_merchant(&pp, b0_merch, &merch_keypair));
    println!(">> TIME for init_merchant: {}", initm_time);

    println!("[5b] libbolt - initialize on the customer side with balance {}", b0_cust);
    let cm_csp = bidirectional::generate_commit_setup(&pp, &merch_keypair.pk);
    let (mut cust_data, initc_time) = measure_ret_mut!(bidirectional::init_customer(&pp, &channel, b0_cust, b0_merch, &cm_csp, &cust_keypair));
    println!(">> TIME for init_customer: {}", initc_time);
    println!("******************************************");
    // libbolt tests below
    println!("Testing the establish protocol...");

    println!("[6a] libbolt - entering the establish protocol for the channel");
    let (proof1, est_cust_time1) = measure!(bidirectional::establish_customer_phase1(&pp, &cust_data, &merch_data.bases));
    println!(">> TIME for establish_customer_phase1: {}", est_cust_time1);

    println!("[6b] libbolt - obtain the wallet signature from the merchant");
    let (wallet_sig, est_merch_time2) = measure!(bidirectional::establish_merchant_phase2(&pp, &mut channel, &merch_data, &proof1));
    println!(">> TIME for establish_merchant_phase2: {}", est_merch_time2);

    println!("[6c] libbolt - complete channel establishment");
    assert!(bidirectional::establish_customer_final(&pp, &merch_keypair.pk, &mut cust_data.csk, wallet_sig));

    assert!(channel.channel_established);

    println!("Channel has been established!");
    println!("******************************************");

    println!("Testing the pay protocol...");
    // let's test the pay protocol
    bidirectional::pay_by_customer_phase1_precompute(&pp, &cust_data.channel_token, &merch_keypair.pk, &mut cust_data.csk);
    let s = PreciseTime::now();
    let (t_c, new_wallet, pay_proof) = bidirectional::pay_by_customer_phase1(&pp, &channel, &cust_data.channel_token, // channel token
                                                                        &merch_keypair.pk, // merchant pub key
                                                                        &cust_data.csk, // wallet
                                                                        5); // balance increment
    let e = PreciseTime::now();
    println!(">> TIME for pay_by_customer_phase1: {}", s.to(e));

    // get the refund token (rt_w)
    let (rt_w, pay_merch_time1) = measure!(bidirectional::pay_by_merchant_phase1(&pp, &mut channel, &pay_proof, &merch_data));
    println!(">> TIME for pay_by_merchant_phase1: {}", pay_merch_time1);

    // get the revocation token (rv_w) on the old public key (wpk)
    let (rv_w, pay_cust_time2) = measure!(bidirectional::pay_by_customer_phase2(&pp, &cust_data.csk, &new_wallet, &merch_keypair.pk, &rt_w));
    println!(">> TIME for pay_by_customer_phase2: {}", pay_cust_time2);

    // get the new wallet sig (new_wallet_sig) on the new wallet
    let (new_wallet_sig, pay_merch_time2) = measure!(bidirectional::pay_by_merchant_phase2(&pp, &mut channel, &pay_proof, &mut merch_data, &rv_w));
    println!(">> TIME for pay_by_merchant_phase2: {}", pay_merch_time2);

    assert!(bidirectional::pay_by_customer_final(&pp, &merch_keypair.pk, &mut cust_data, t_c, new_wallet, new_wallet_sig));

    {
        // scope localizes the immutable borrow here (for debug purposes only)
        let cust_wallet = &cust_data.csk;
        let merch_wallet = &merch_data.csk;
        println!("Customer balance: {}", cust_wallet.balance);
        println!("Merchant balance: {}", merch_wallet.balance);
    }

    bidirectional::pay_by_customer_phase1_precompute(&pp, &cust_data.channel_token, &merch_keypair.pk, &mut cust_data.csk);
    let (t_c1, new_wallet1, pay_proof1) = bidirectional::pay_by_customer_phase1(&pp, &channel, &cust_data.channel_token, // channel token
                                                                        &merch_keypair.pk, // merchant pub key
                                                                        &cust_data.csk, // wallet
                                                                        -10); // balance increment

    // get the refund token (rt_w)
    let rt_w1 = bidirectional::pay_by_merchant_phase1(&pp, &mut channel, &pay_proof1, &merch_data);

    // get the revocation token (rv_w) on the old public key (wpk)
    let rv_w1 = bidirectional::pay_by_customer_phase2(&pp, &cust_data.csk, &new_wallet1, &merch_keypair.pk, &rt_w1);

    // get the new wallet sig (new_wallet_sig) on the new wallet
    let new_wallet_sig1 = bidirectional::pay_by_merchant_phase2(&pp, &mut channel, &pay_proof1, &mut merch_data, &rv_w1);

    assert!(bidirectional::pay_by_customer_final(&pp, &merch_keypair.pk, &mut cust_data, t_c1, new_wallet1, new_wallet_sig1));

    {
        let cust_wallet = &cust_data.csk;
        let merch_wallet = &merch_data.csk;
        println!("Updated balances...");
        println!("Customer balance: {}", cust_wallet.balance);
        println!("Merchant balance: {}", merch_wallet.balance);
        let updated_cust_bal = b0_cust + 5;
        let updated_merch_bal = b0_merch - 5;
        assert_eq!(updated_cust_bal, cust_wallet.balance);
        assert_eq!(updated_merch_bal, merch_wallet.balance);
    }
    println!("Pay protocol complete!");

    println!("******************************************");
    println!("Testing the dispute algorithms...");

    {
        let cust_wallet = &cust_data.csk;
        // get channel closure message
        let rc_c = bidirectional::customer_refund(&pp, &channel, &merch_keypair.pk, &cust_wallet);
        println!("Obtained the channel closure message: {}", rc_c.message.msgtype);

        let channel_token = &cust_data.channel_token;
        let rc_m = bidirectional::merchant_refute(&pp, &mut channel, &channel_token, &merch_data, &rc_c, &rv_w1.signature);
        println!("Merchant has refuted the refund request!");

        let (new_b0_cust, new_b0_merch) = bidirectional::resolve(&pp, &cust_data, &merch_data,
                                                                 Some(rc_c), Some(rc_m), Some(rt_w1));
        println!("Resolved! Customer = {}, Merchant = {}", new_b0_cust, new_b0_merch);
    }

    // TODO: add tests for customer/merchant cheating scenarios
    println!("******************************************");
}
