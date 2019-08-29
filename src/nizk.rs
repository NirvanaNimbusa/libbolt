extern crate pairing;
extern crate rand;

use super::*;
use rand::Rng;
use cl::{KeyPair, Signature, PublicParams, setup, BlindKeyPair, ProofState, SignatureProof, BlindPublicKey};
use ped92::{CSParams, Commitment, CSMultiParams, CommitmentProof};
use pairing::{Engine, CurveProjective};
use ff::PrimeField;
use wallet::Wallet;
use ccs08::{RPPublicParams, RangeProof};
use serde::{Serialize, Deserialize};
use util;
use std::borrow::BorrowMut;

/// NIZKProof is the object that represents the NIZK Proof of Knowledge during the payment and closing protocol
#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "<E as ff::ScalarEngine>::Fr: serde::Serialize, \
<E as pairing::Engine>::G1: serde::Serialize, \
<E as pairing::Engine>::G2: serde::Serialize, \
<E as pairing::Engine>::Fqk: serde::Serialize"
))]
#[serde(bound(deserialize = "<E as ff::ScalarEngine>::Fr: serde::Deserialize<'de>, \
<E as pairing::Engine>::G1: serde::Deserialize<'de>, \
<E as pairing::Engine>::G2: serde::Deserialize<'de>, \
<E as pairing::Engine>::Fqk: serde::Deserialize<'de>"
))]
pub struct NIZKProof<E: Engine> {
    pub sig: Signature<E>,
    pub sigProof: SignatureProof<E>,
    pub comProof: CommitmentProof<E>,
    pub rpBC: RangeProof<E>,
    pub rpBM: RangeProof<E>,
}

/// NIZKPublicParams are public parameters to perform a NIZK Proof of Knowledge during the payment and closing protocol
#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(serialize = "<E as ff::ScalarEngine>::Fr: serde::Serialize, \
<E as pairing::Engine>::G1: serde::Serialize, \
<E as pairing::Engine>::G2: serde::Serialize"
))]
#[serde(bound(deserialize = "<E as ff::ScalarEngine>::Fr: serde::Deserialize<'de>, \
<E as pairing::Engine>::G1: serde::Deserialize<'de>, \
<E as pairing::Engine>::G2: serde::Deserialize<'de>"
))]
pub struct NIZKPublicParams<E: Engine> {
    pub mpk: PublicParams<E>,
    pub keypair: BlindKeyPair<E>,
    pub comParams: CSMultiParams<E>,
    pub rpParamsBC: RPPublicParams<E>,
    pub rpParamsBM: RPPublicParams<E>,
}

impl<E: Engine> NIZKPublicParams<E> {
    /// Basic setup for the NIZKPublicParams
    /// Takes as input a random generator and the length of the message which should be 4 during payment protocol and 5 for the closing protocol
    pub fn setup<R: Rng>(rng: &mut R, messageLength: usize) -> Self {
        let mpk = setup(rng);
        let keypair = BlindKeyPair::<E>::generate(rng, &mpk, messageLength);
        let comParams = keypair.generate_cs_multi_params(&mpk);
        let rpParamsBC = RPPublicParams::setup(rng, 0, std::i16::MAX as i32, comParams.clone());
        let rpParamsBM = RPPublicParams::setup(rng, 0, std::i16::MAX as i32, comParams.clone());

        NIZKPublicParams { mpk, keypair, comParams, rpParamsBC, rpParamsBM }
    }

    /** This method can be called to create the proof during the payment and closing protocol
        Input:
        rng: random generator
        r: randomness of commitment of old wallet (TODO: still necessary?)
        oldWallet: This is the wallet before payment occurs
        newWallet: This is the new state of the wallet after payment
        newWalletCom: A commitment of the new wallet
        rPrime: randomness of commitment of new wallet
        paymentToken: A blind signature on the old wallet
        Output:
        NIZKProof: a proof that can be verified by the merchant during payment or closing protocol
    */
    pub fn prove<R: Rng>(&self, rng: &mut R, r: E::Fr, oldWallet: Wallet<E>, newWallet: Wallet<E>,
                         newWalletCom: Commitment<E>, rPrime: E::Fr, paymentToken: &Signature<E>) -> NIZKProof<E> {
        //Commitment phase
        //commit commitment
        let w_len = newWallet.as_fr_vec().len();
        let diff = self.comParams.pub_bases.len() - w_len;
        let max = match diff > 1 {
            true => w_len,
            false => self.comParams.pub_bases.len()
        };

        let (D, t, rt, mut reveal_wallet) = CommitmentProof::<E>::prove_commitment(rng, &self.comParams, &newWallet.as_fr_vec(), &vec! {});

        //commit signature
        let zero = E::Fr::zero();
        let tOptional = match max > 4 {
            true => Some(vec!(t[1], zero, t[3].clone(), t[4].clone())),
            false => Some(vec!(t[1], zero, t[3].clone()))
        };
        let proofState = self.keypair.prove_commitment(rng, &self.mpk, &paymentToken, tOptional, None);

        //commit range proof
        let rpStateBC = self.rpParamsBC.prove_commitment(rng, newWallet.bc.clone(), newWalletCom.clone(), 3, None, None);
        let rpStateBM = self.rpParamsBM.prove_commitment(rng, newWallet.bm.clone(), newWalletCom.clone(), 4, None, None);

        //Compute challenge
        let challenge = NIZKPublicParams::<E>::hash(proofState.a, vec! {D, rpStateBC.ps1.D, rpStateBC.ps2.D, rpStateBM.ps1.D, rpStateBM.ps2.D});

        //Response phase
        //response for signature
        let oldWalletVec = oldWallet.as_fr_vec();
        let sigProof = self.keypair.prove_response(&proofState, challenge, &mut oldWalletVec.clone());

        //response commitment
        let newWalletVec = newWallet.as_fr_vec();
        let comProof = CommitmentProof::<E>::prove_response(&newWalletVec, &rPrime, &vec! {}, D, &t, rt, reveal_wallet.borrow_mut(), &challenge);

        //response range proof
        let mut vec01 = newWalletVec[0..2].to_vec();
        let mut vecWithout2 = vec01.clone();
        let mut vec3 = newWalletVec[3..].to_vec();
        vecWithout2.append(&mut vec3);
        let vec2 = newWalletVec[2].clone();
        vec01.push(vec2);
        if newWalletVec.len() > 4 {
            let mut vec4 = newWalletVec[4..].to_vec();
            vec01.append(&mut vec4);
        }
        let rpBC = self.rpParamsBC.prove_response(rPrime.clone(), &rpStateBC, challenge.clone(), 3, vecWithout2.to_vec());
        let rpBM = self.rpParamsBM.prove_response(rPrime.clone(), &rpStateBM, challenge.clone(), 4, vec01.to_vec());

        NIZKProof { sig: proofState.blindSig, sigProof, comProof, rpBC, rpBM }
    }

    /**
        Verify a NIZK Proof of Knowledge during payment or closing protocl
        Input:
        proof: A NIZK proof created by the Customer
        epsilon: The transaction amount of the payment
        com: Commitment of the new wallet that needs to be signed
        wpk: reveal of wallet public key of the old wallet.
    */
    pub fn verify(&self, proof: NIZKProof<E>, epsilon: E::Fr, com: &Commitment<E>, wpk: E::Fr) -> bool {
        //verify signature is not the identity
        let r0 = proof.sig.h != E::G1::one();

        //compute challenge
        let challenge = NIZKPublicParams::<E>::hash(proof.sigProof.a, vec! {proof.comProof.T, proof.rpBC.p1.D, proof.rpBC.p2.D, proof.rpBM.p1.D, proof.rpBM.p2.D});

        //verify knowledge of signature
        let mut r1 = self.keypair.public.verify_proof(&self.mpk, proof.sig, proof.sigProof.clone(), challenge);
        let mut wpkc = wpk.clone();
        wpkc.mul_assign(&challenge.clone());
        r1 = r1 && proof.sigProof.zsig[1] == wpkc;

        //verify knowledge of commitment
        let r2 = proof.comProof.verify_proof(&self.comParams, &com.c.clone(), &challenge);

        //verify range proofs
        let r3 = self.rpParamsBC.verify(proof.rpBC.clone(), challenge.clone(), 3);
        let r4 = self.rpParamsBM.verify(proof.rpBM.clone(), challenge.clone(), 4);

        //verify linear relationship
        let mut r5 = proof.comProof.z[1] == proof.sigProof.zsig[0];
        let mut zsig2 = proof.sigProof.zsig[2].clone();
        let mut epsC = epsilon.clone();
        epsC.mul_assign(&challenge.clone());
        zsig2.sub_assign(&epsC.clone());
        r5 = r5 && proof.comProof.z[3] == zsig2;
        let mut zsig3 = proof.sigProof.zsig[3].clone();
        zsig3.add_assign(&epsC.clone());
        r5 = r5 && proof.comProof.z[4] == zsig3;

        r0 && r1 && r2 && r3 && r4 && r5
    }

    fn hash(a: E::Fqk, T: Vec<E::G1>) -> E::Fr {
        let mut x_vec: Vec<u8> = Vec::new();
        x_vec.extend(format!("{}", a).bytes());
        for t in T {
            x_vec.extend(format!("{}", t).bytes());
        }

        util::hash_to_fr::<E>(x_vec)
    }
}

///
/// Verify PoK for the opening of a commitment during the establishment protocol
///
pub fn verify_opening<E: Engine>(com_params: &CSMultiParams<E>, com: &E::G1, proof: &CommitmentProof<E>, init_cust: i32, init_merch: i32) -> bool {
    let xvec: Vec<E::G1> = vec![proof.T.clone(), com.clone()];
    let challenge = util::hash_g1_to_fr::<E>(&xvec);

    // compute the
    let com_equal = proof.verify_proof(com_params, com, &challenge);

    if proof.index.len() == 0 {
        println!("verify_opening - doing any partial reveals?");
        return false;
    }

    // verify linear relationshps
    // pkc: index = 1
    let mut s1 = proof.reveal[1].clone();
    s1.mul_assign(&challenge);
    s1.add_assign(&proof.t[1]);
    let pkc_equal = (s1 == proof.z[1]);

    // cust init balances: index = 3
    let mut s3 = proof.reveal[3].clone();
    s3.mul_assign(&challenge);
    s3.add_assign(&proof.t[3]);
    let init_c = util::convert_int_to_fr::<E>(init_cust);
    let bc_equal = (s3 == proof.z[3]) && (proof.reveal[3] == init_c);

    // merch init balances: index = 4
    let mut s4 = proof.reveal[4].clone();
    s4.mul_assign(&challenge);
    s4.add_assign(&proof.t[4]);
    let init_m = util::convert_int_to_fr::<E>(init_merch);
    let bm_equal = (s4 == proof.z[4]) && (proof.reveal[4] == init_m);

    return com_equal && pkc_equal && bc_equal && bm_equal;
}


#[cfg(test)]
mod tests {
    use super::*;
    use pairing::bls12_381::{Bls12, Fr};
    use util::convert_int_to_fr;

    #[test]
    fn nizk_proof_works() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let wpkprime = Fr::rand(rng);
        let bc = rng.gen_range(100, 1000);
        let mut bc2 = bc.clone();
        let bm = rng.gen_range(100, 1000);
        let mut bm2 = bm.clone();
        let epsilon = &rng.gen_range(1, 100);
        bc2 -= epsilon;
        bm2 += epsilon;
        let r = Fr::rand(rng);
        let rprime = Fr::rand(rng);

        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 4);
        let wallet1 = Wallet { pkc, wpk, bc, bm, close: None };
        let commitment1 = pubParams.comParams.commit(&wallet1.as_fr_vec(), &r);
        let wallet2 = Wallet { pkc, wpk: wpkprime, bc: bc2, bm: bm2, close: None };
        let commitment2 = pubParams.comParams.commit(&wallet2.as_fr_vec(), &rprime);
        let blindPaymentToken = pubParams.keypair.sign_blind(rng, &pubParams.mpk, commitment1.clone());
        let paymentToken = pubParams.keypair.unblind(&r, &blindPaymentToken);

        let proof = pubParams.prove(rng, r, wallet1, wallet2,
                                    commitment2.clone(), rprime, &paymentToken);
        let fr = convert_int_to_fr::<Bls12>(*epsilon);
        assert_eq!(pubParams.verify(proof, fr, &commitment2, wpk), true);
    }

    #[test]
    fn nizk_proof_negative_value_works() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let wpkprime = Fr::rand(rng);
        let bc = rng.gen_range(100, 1000);
        let mut bc2 = bc.clone();
        let bm = rng.gen_range(100, 1000);
        let mut bm2 = bm.clone();
        let epsilon = &rng.gen_range(-100, -1);
        bc2 -= epsilon;
        bm2 += epsilon;
        let r = Fr::rand(rng);
        let rprime = Fr::rand(rng);

        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 4);
        let wallet1 = Wallet { pkc, wpk, bc, bm, close: None };
        let commitment1 = pubParams.comParams.commit(&wallet1.as_fr_vec(), &r);
        let wallet2 = Wallet { pkc, wpk: wpkprime, bc: bc2, bm: bm2, close: None };
        let commitment2 = pubParams.comParams.commit(&wallet2.as_fr_vec(), &rprime);
        let blindPaymentToken = pubParams.keypair.sign_blind(rng, &pubParams.mpk, commitment1.clone());
        let paymentToken = pubParams.keypair.unblind(&r, &blindPaymentToken);

        let proof = pubParams.prove(rng, r, wallet1, wallet2,
                                    commitment2.clone(), rprime, &paymentToken);
        let fr = convert_int_to_fr::<Bls12>(*epsilon);
        assert_eq!(pubParams.verify(proof, fr, &commitment2, wpk), true);
    }

    #[test]
    fn nizk_proof_close_works() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let wpkprime = Fr::rand(rng);
        let bc = rng.gen_range(100, 1000);
        let mut bc2 = bc.clone();
        let bm = rng.gen_range(100, 1000);
        let mut bm2 = bm.clone();
        let epsilon = &rng.gen_range(1, 100);
        bc2 -= epsilon;
        bm2 += epsilon;
        let r = Fr::rand(rng);
        let rprime = Fr::rand(rng);

        let _closeToken = Fr::rand(rng);
        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 5);
        let wallet1 = Wallet { pkc, wpk, bc, bm, close: None };
        let commitment1 = pubParams.comParams.commit(&wallet1.as_fr_vec(), &r);
        let wallet2 = Wallet { pkc, wpk: wpkprime, bc: bc2, bm: bm2, close: Some(_closeToken) };
        let commitment2 = pubParams.comParams.commit(&wallet2.as_fr_vec(), &rprime);
        let blindPaymentToken = pubParams.keypair.sign_blind(rng, &pubParams.mpk, commitment1.clone());
        let paymentToken = pubParams.keypair.unblind(&r, &blindPaymentToken);

        let blindCloseToken = pubParams.keypair.sign_blind(rng, &pubParams.mpk, commitment2.clone());
        let closeToken = pubParams.keypair.unblind(&rprime, &blindCloseToken);

        // verify the blind signatures
        let pk = pubParams.keypair.get_public_key(&pubParams.mpk);
        assert!(pk.verify(&pubParams.mpk, &wallet1.as_fr_vec(), &paymentToken));

        println!("close => {}", &wallet2);
        assert!(pk.verify(&pubParams.mpk, &wallet2.as_fr_vec(), &closeToken));

        let proof = pubParams.prove(rng, r, wallet1, wallet2,
                                    commitment2.clone(), rprime, &paymentToken);

        assert_eq!(pubParams.verify(proof, Fr::from_str(&epsilon.to_string()).unwrap(), &commitment2, wpk), true);
    }

    #[test]
    fn nizk_proof_false_statements() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let wpkprime = Fr::rand(rng);
        let bc = rng.gen_range(100, 1000);
        let mut bc2 = bc.clone();
        let bm = rng.gen_range(100, 1000);
        let mut bm2 = bm.clone();
        let epsilon = &rng.gen_range(1, 100);
        bc2 -= epsilon;
        bm2 += epsilon;
        let r = Fr::rand(rng);
        let rprime = Fr::rand(rng);

        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 4);
        let wallet1 = Wallet { pkc, wpk, bc, bm, close: None };
        let wallet2 = Wallet::<Bls12> { pkc, wpk: wpkprime, bc: bc2, bm: bm2, close: None };

        let bc2Prime = bc.clone();
        let wallet3 = Wallet { pkc, wpk: wpkprime, bc: bc2Prime, bm: bm2, close: None };
        let commitment1 = pubParams.comParams.commit(&wallet1.as_fr_vec().clone(), &r);
        let commitment2 = pubParams.comParams.commit(&wallet3.as_fr_vec(), &rprime);
        let blindPaymentToken = pubParams.keypair.sign_blind(rng, &pubParams.mpk, commitment1.clone());
        let paymentToken = pubParams.keypair.unblind(&r, &blindPaymentToken);
        let proof = pubParams.prove(rng, r, wallet1.clone(), wallet3, commitment2.clone(), rprime, &paymentToken);
        assert_eq!(pubParams.verify(proof, Fr::from_str(&epsilon.to_string()).unwrap(), &commitment2, wpk), false);

        let bm2Prime = bm.clone();
        let wallet4 = Wallet { pkc, wpk: wpkprime, bc: bc2, bm: bm2Prime, close: None };
        let commitment2 = pubParams.comParams.commit(&wallet4.as_fr_vec(), &rprime);
        let proof = pubParams.prove(rng, r, wallet1.clone(), wallet4, commitment2.clone(), rprime, &paymentToken);
        assert_eq!(pubParams.verify(proof, Fr::from_str(&epsilon.to_string()).unwrap(), &commitment2, wpk), false);

        let wallet5 = Wallet { pkc: Fr::rand(rng), wpk: wpkprime, bc: bc2, bm: bm2, close: None };
        let commitment2 = pubParams.comParams.commit(&wallet5.as_fr_vec(), &rprime);
        let proof = pubParams.prove(rng, r, wallet1.clone(), wallet5, commitment2.clone(), rprime, &paymentToken);
        assert_eq!(pubParams.verify(proof, Fr::from_str(&epsilon.to_string()).unwrap(), &commitment2, wpk), false);
    }

    #[test]
    fn nizk_proof_commitment_opening_works() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let t = Fr::rand(rng);

        let bc = rng.gen_range(100, 1000);
        let bm = rng.gen_range(100, 1000);
        let wallet = Wallet::<Bls12> { pkc: pkc, wpk: wpk, bc: bc, bm: bm, close: None };

        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 4);
        let com = pubParams.comParams.commit(&wallet.as_fr_vec().clone(), &t);

        let com_proof = CommitmentProof::<Bls12>::new(rng, &pubParams.comParams,
                                                      &com.c, &wallet.as_fr_vec(), &t, &vec![1, 3, 4]);

        assert!(verify_opening(&pubParams.comParams, &com.c, &com_proof, bc, bm));
    }

    #[test]
    fn nizk_proof_false_commitment() {
        let rng = &mut rand::thread_rng();
        let pkc = Fr::rand(rng);
        let wpk = Fr::rand(rng);
        let t = Fr::rand(rng);

        let bc = rng.gen_range(100, 1000);
        let bc2 = rng.gen_range(100, 1000);
        let bm = rng.gen_range(100, 1000);
        let wallet1 = Wallet::<Bls12> { pkc: pkc, wpk: wpk, bc: bc, bm: bm, close: None };
        let wallet2 = Wallet::<Bls12> { pkc: pkc, wpk: wpk, bc: bc2, bm: bm, close: None };

        let pubParams = NIZKPublicParams::<Bls12>::setup(rng, 4);
        let com1 = pubParams.comParams.commit(&wallet1.as_fr_vec().clone(), &t);
        let com2 = pubParams.comParams.commit(&wallet2.as_fr_vec().clone(), &t);

        let com1_proof = CommitmentProof::<Bls12>::new(rng, &pubParams.comParams,
                                                       &com1.c, &wallet1.as_fr_vec(), &t, &vec![1, 3, 4]);

        assert!(verify_opening(&pubParams.comParams, &com1.c, &com1_proof, bc, bm));
        assert!(!verify_opening(&pubParams.comParams, &com2.c, &com1_proof, bc2, bm));
    }


    #[test]
    fn test_nizk_serialization() {
        let mut rng = &mut rand::thread_rng();

        let l = 5;
        let mpk = setup(&mut rng);
        let blindkeypair = BlindKeyPair::<Bls12>::generate(&mut rng, &mpk, l);
        let comParams = blindkeypair.generate_cs_multi_params(&mpk);
        let rpParamsBC = ccs08::RPPublicParams::setup(rng, 0, std::i16::MAX as i32, comParams.clone());
        let rpParamsBM = ccs08::RPPublicParams::setup(rng, 0, std::i16::MAX as i32, comParams.clone());

        let nizk_params = NIZKPublicParams { mpk: mpk, keypair: blindkeypair, comParams: comParams, rpParamsBC: rpParamsBC, rpParamsBM: rpParamsBM };

        let is_serialized = serde_json::to_vec(&nizk_params).unwrap();
        println!("NIZK Struct len: {}", is_serialized.len());

        // deserialize
    }
}
