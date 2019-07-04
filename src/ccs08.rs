/*
Implementation of the ZK Range Proof scheme, based on:
Efficient Protocols for Set Membership and Range Proofs
Jan Camenisch, Rafik Chaabouni, and abhi shelat
Asiacrypt 2008
*/
extern crate pairing;
extern crate rand;

use rand::{thread_rng, Rng};
use super::*;
use cl::{KeyPair, Signature, PublicParams, setup, BlindKeyPair, ProofState, SignatureProof};
use ped92::{CSParams, Commitment};
use pairing::{Engine, CurveProjective};
use ff::PrimeField;
use std::collections::HashMap;
use std::fmt::Display;
use std::mem::transmute;
use util::fmt_bytes_to_int;

/**
paramsUL contains elements generated by the verifier, which are necessary for the prover.
This must be computed in a trusted setup.
*/
#[derive(Clone)]
struct ParamsUL<E: Engine> {
    pub mpk: PublicParams<E>,
    pub signatures: HashMap<String, Signature<E>>,
    pub com: CSParams<E>,
    kp: BlindKeyPair<E>,
    // u determines the amount of signatures we need in the public params.
    // Each signature can be compressed to just 1 field element of 256 bits.
    // Then the parameters have minimum size equal to 256*u bits.
    u: i64,
    // l determines how many pairings we need to compute, then in order to improve
    // verifier`s performance we want to minize it.
    // Namely, we have 2*l pairings for the prover and 3*l for the verifier.
    l: i64,
}

/**
proofUL contains the necessary elements for the ZK range proof with range [0,u^l).
*/
#[derive(Clone)]
struct ProofUL<E: Engine> {
    V: Vec<Signature<E>>,
    D: E::G2,
    comm: Commitment<E>,
    sigProofs: Vec<SignatureProof<E>>,
    ch: E::Fr,
    zr: E::Fr,
}

/**
RangeProof contains the necessary elements for the ZK range proof.
*/
#[derive(Clone)]
pub struct RangeProof<E: Engine> {
    p1: ProofUL<E>,
    p2: ProofUL<E>,
}

/**
params contains elements generated by the verifier, which are necessary for the prover.
This must be computed in a trusted setup.
*/
#[derive(Clone)]
pub struct RPPublicParams<E: Engine> {
    p: ParamsUL<E>,
    a: i64,
    b: i64,
}

impl<E: Engine> ParamsUL<E> {
    /**
        setup_ul generates the signature for the interval [0,u^l).
        The value of u should be roughly b/log(b), but we can choose smaller values in
        order to get smaller parameters, at the cost of having worse performance.
    */
    pub fn setup_ul<R: Rng>(rng: &mut R, u: i64, l: i64) -> Self {
        let mpk = setup(rng);
        let kp = BlindKeyPair::<E>::generate(rng, &mpk, 1);

        let mut signatures: HashMap<String, Signature<E>> = HashMap::new();
        for i in 0..u {
            let sig_i = kp.sign(rng, &vec! {E::Fr::from_str(i.to_string().as_str()).unwrap()});
            signatures.insert(i.to_string(), sig_i);
        }

        let com = CSParams::setup(rng);
        return ParamsUL { mpk, signatures, com, kp, u, l };
    }

    /**
        prove_ul method is used to produce the ZKRP proof that secret x belongs to the interval [0,U^L).
    */
    pub fn prove_ul<R: Rng>(&self, rng: &mut R, x: i64, r: E::Fr) -> ProofUL<E> {
        if x > self.u.pow(self.l as u32) || x < 0 {
            panic!("x is not within the range.");
        }
        let decx = decompose(x, self.u, self.l);
        let modx = E::Fr::from_str(&(x.to_string())).unwrap();

        // Initialize variables
        let mut proofStates = Vec::<ProofState<E>>::with_capacity(self.l as usize);
        let mut sigProofs = Vec::<SignatureProof<E>>::with_capacity(self.l as usize);
        let mut V = Vec::<Signature<E>>::with_capacity(self.l as usize);
        let mut D = E::G2::zero();
        let m = E::Fr::rand(rng);

        // D = H^m
        let mut hm = self.com.h.clone();
        hm.mul_assign(m);
        for i in 0..self.l as usize {
            let signature = self.signatures.get(&decx[i].to_string()).unwrap();
            let proofState = self.kp.prove_commitment(rng, &self.mpk, &signature);

            V.push(proofState.blindSig.clone());
            proofStates.push(proofState);

            let ui = self.u.pow(i as u32);
            let mut aux = self.com.g.clone();
            for j in 0..self.kp.public.Y2.len() {
                let mut muiti = proofStates[i].t[j].clone();
                muiti.mul_assign(&E::Fr::from_str(&ui.to_string()).unwrap());
                aux.mul_assign(muiti);
            }
            D.add_assign(&aux);
        }
        D.add_assign(&hm);

        let C = self.com.commit(rng, modx, Some(r));
        // Fiat-Shamir heuristic
        let c = hash::<E>(proofStates.clone(), D.clone());

        let mut zr = m.clone();
        let mut rc = r.clone();
        rc.mul_assign(&c);
        zr.add_assign(&rc);
        for i in 0..self.l as usize {
            let mut dx = E::Fr::from_str(&decx[i].to_string()).unwrap();

            let proof = self.kp.prove_response(&proofStates[i].clone(), c, &mut vec!{dx});

            sigProofs.push(proof);
        }

        return ProofUL { V, D, comm: C, sigProofs, ch: c, zr };
    }

    /**
        verify_ul is used to validate the ZKRP proof. It returns true iff the proof is valid.
    */
    pub fn verify_ul(&self, proof: &ProofUL<E>) -> bool {
        // D == C^c.h^ zr.g^zsig ?
        let r1 = self.verify_part1(&proof);
        let r2 = self.verify_part2(&proof);
        return r1 && r2;
    }

    fn verify_part2(&self, proof: &ProofUL<E>) -> bool {
        let mut r2 = true;
        for i in 0..self.l as usize {
            let subResult = self.kp.public.verify_proof(&self.mpk, proof.V[i].clone(), proof.sigProofs[i].clone(), proof.ch);

            r2 = r2 && subResult;
        }
        r2
    }

    fn verify_part1(&self, proof: &ProofUL<E>) -> bool {
        let mut D = proof.comm.c.clone();
        D.mul_assign(proof.ch);
        D.negate();
        let mut hzr = self.com.h.clone();
        hzr.mul_assign(proof.zr);
        D.add_assign(&hzr);
        for i in 0..self.l as usize {
            let ui = self.u.pow(i as u32);
            let mut aux = self.com.g.clone();
            for j in 0..self.kp.public.Y2.len() {
                let mut muizsigi = proof.sigProofs[i].zsig[j];
                muizsigi.mul_assign(&E::Fr::from_str(&ui.to_string()).unwrap());
                aux.mul_assign(muizsigi);
            }
            D.add_assign(&aux);
        }
        return D == proof.D;
    }
}

fn hash<E: Engine>(a: Vec<ProofState<E>>, D: E::G2) -> E::Fr {
    // create a Sha256 object
    let mut a_vec: Vec<u8> = Vec::new();
    for a_el in a {
        a_vec.extend(format!("{}", a_el.a).bytes());
    }

    let mut x_vec: Vec<u8> = Vec::new();
    x_vec.extend(format!("{}", D).bytes());
    a_vec.extend(x_vec);
    let sha2_digest = sha512::hash(a_vec.as_slice());

    let mut hash_buf: [u8; 64] = [0; 64];
    hash_buf.copy_from_slice(&sha2_digest[0..64]);
    let hexresult = fmt_bytes_to_int(hash_buf);
    let result = E::Fr::from_str(&hexresult);
    return result.unwrap();
}

/*
Decompose receives as input an integer x and outputs an array of integers such that
x = sum(xi.u^i), i.e. it returns the decomposition of x into base u.
*/
fn decompose(x: i64, u: i64, l: i64) -> Vec<i64> {
    let mut result = Vec::with_capacity(l as usize);
    let mut decomposer = x.clone();
    for _i in 0..l {
        result.push(decomposer % u);
        decomposer = decomposer / u;
    }
    return result;
}

impl<E: Engine> RPPublicParams<E> {
    /**
        Setup receives integers a and b, and configures the parameters for the rangeproof scheme.
    */
    pub fn setup<R: Rng>(rng: &mut R, a: i64, b: i64) -> Self {
        // Compute optimal values for u and l
        if a > b {
            panic!("a must be less than or equal to b");
        }
        //TODO: optimize u?
        let logb = (b as f64).log2();
        let loglogb = logb.log2();
        if loglogb > 0.0 {
            let mut u = (logb / loglogb) as i64;
            if u < 2 {
                u = 2;
            }
            let l = (b as f64).log(u as f64).ceil() as i64;
            let params_out: ParamsUL<E> = ParamsUL::<E>::setup_ul(rng, u, l);
            return RPPublicParams { p: params_out, a, b };
        } else {
            panic!("log(log(b)) is zero");
        }
    }

    /**
        Prove method is responsible for generating the zero knowledge range proof.
    */
    pub fn prove<R: Rng>(&self, rng: &mut R, x: i64) -> RangeProof<E> {
        if x > self.b || x < self.a {
            panic!("x is not within the range.");
        }
        let ul = self.p.u.pow(self.p.l as u32);
        let r = E::Fr::rand(rng);

        // x - b + ul
        let xb = x - self.b + ul;
        let first = self.p.prove_ul(rng, xb, r);

        // x - a
        let xa = x - self.a;
        let second = self.p.prove_ul(rng, xa, r);

        return RangeProof { p1: first, p2: second };
    }

    /**
        Verify is responsible for validating the range proof.
    */
    pub fn verify(&self, proof: RangeProof<E>) -> bool {
        let first = self.p.verify_ul(&proof.p1);
        let second = self.p.verify_ul(&proof.p2);
        return first && second;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use pairing::bls12_381::{Bls12, G1, G2, Fq12, Fr};
    use time::PreciseTime;
    use std::ops::Add;
    use core::mem;

    #[test]
    fn setup_ul_works() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 3);
        assert_eq!(params.signatures.len(), 2);
        for (m, s) in params.signatures {
            assert_eq!(params.kp.verify(&params.mpk, &vec! {Fr::from_str(m.to_string().as_str()).unwrap()}, &Fr::zero(), &s), true);
        }
    }

    #[test]
    fn prove_ul_works() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 4);
        let fr = Fr::rand(rng);
        let proof = params.prove_ul(rng, 10, fr);
        assert_eq!(proof.V.len(), 4);
        assert_eq!(proof.sigProofs.len(), 4);
    }

    #[test]
    #[should_panic(expected = "x is not within the range")]
    fn prove_ul_not_in_range() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 3);
        let fr = Fr::rand(rng);
        params.prove_ul(rng, 100, fr);
    }

    #[test]
    fn prove_and_verify_part1_ul_works() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 4);
        let fr = Fr::rand(rng);
        let proof = params.prove_ul(rng, 10, fr);
        assert_eq!(params.verify_part1(&proof), true);
    }

    #[test]
    fn prove_and_verify_part2_ul_works() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 4);
        let fr = Fr::rand(rng);
        let proof = params.prove_ul(rng, 10, fr);
        assert_eq!(params.verify_part2(&proof), true);
    }

    #[test]
    fn prove_and_verify_ul_works() {
        let rng = &mut rand::thread_rng();
        let params = ParamsUL::<Bls12>::setup_ul(rng, 2, 4);
        let fr = Fr::rand(rng);
        let proof = params.prove_ul(rng, 10, fr);
        assert_eq!(params.verify_ul(&proof), true);
    }

    #[test]
    fn prove_and_verify_works() {
        let rng = &mut rand::thread_rng();
        let params = RPPublicParams::<Bls12>::setup(rng, 2, 25);
        let proof = params.prove(rng, 10);
        assert_eq!(params.verify(proof), true);
    }

    #[test]
    #[should_panic(expected = "x is not within the range")]
    fn prove_not_in_range() {
        let rng = &mut rand::thread_rng();
        let params = RPPublicParams::<Bls12>::setup(rng, 2, 25);
        let proof = params.prove(rng, 26);
    }

    #[test]
    #[ignore]
    fn prove_and_verify_performance() {
        let rng = &mut rand::thread_rng();
        let mut averageSetup = time::Duration::nanoseconds(0);
        let mut averageSetupSize = 0;
        let mut averageProve = time::Duration::nanoseconds(0);
        let mut averageProofSize = 0;
        let mut averageVerify = time::Duration::nanoseconds(0);
        let iter = 5;
        for i in 0..iter {
            let a = rng.gen_range(0, 1000000);
            let b = rng.gen_range(a, 1000000);
            let x = rng.gen_range(a, b);

            let sSetup = PreciseTime::now();
            let params = RPPublicParams::<Bls12>::setup(rng, a, b);
            averageSetup = averageSetup.add(sSetup.to(PreciseTime::now()));
            averageSetupSize += mem::size_of_val(&params);

            let sProve = PreciseTime::now();
            let proof = params.prove(rng, x);
            averageProve = averageProve.add(sProve.to(PreciseTime::now()));
            averageProofSize += mem::size_of_val(&proof);

            let sVerify = PreciseTime::now();
            params.verify(proof);
            averageVerify = averageVerify.add(sVerify.to(PreciseTime::now()));
        }
        print!("Setup: {}\n", averageSetup.num_milliseconds() / iter);
        print!("Setup size: {}\n", averageSetupSize / iter as usize);
        print!("Prove: {}\n", averageProve.num_milliseconds() / iter);
        print!("Proof size: {}\n", averageProofSize / iter as usize);
        print!("Verify: {}\n", averageVerify.num_milliseconds() / iter);
    }

    #[test]
    fn decompose_works() {
        assert_eq!(decompose(25, 3, 3), vec! {1, 2, 2});
        assert_eq!(decompose(336, 7, 3), vec! {0, 6, 6});
        assert_eq!(decompose(285, 8, 3), vec! {5, 3, 4});
        assert_eq!(decompose(125, 13, 2), vec! {8, 9});
        assert_eq!(decompose(143225, 6, 7), vec! {5, 2, 0, 3, 2, 0, 3});
    }

    #[test]
    fn decompose_recompose_works() {
        let vec1 = decompose(25, 3, 5);
        let mut result = 0;
        for i in 0..5 {
            result += vec1[i] * 3i64.pow(i as u32);
        }
        assert_eq!(result, 25);

        let vec1 = decompose(143225, 6, 7);
        let mut result = 0;
        for i in 0..7 {
            result += vec1[i] * 6i64.pow(i as u32);
        }
        assert_eq!(result, 143225);
    }

    #[test]
    fn setup_works() {
        let rng = &mut rand::thread_rng();
        let public_params = RPPublicParams::<Bls12>::setup(rng, 2, 10);
        assert_eq!(public_params.a, 2);
        assert_eq!(public_params.b, 10);
        assert_eq!(public_params.p.signatures.len(), 2);
        assert_eq!(public_params.p.u, 2);
        assert_eq!(public_params.p.l, 4);
        for (m, s) in public_params.p.signatures {
            assert_eq!(public_params.p.kp.verify(&public_params.p.mpk, &vec! {Fr::from_str(m.to_string().as_str()).unwrap()}, &Fr::zero(), &s), true);
        }
    }

    #[test]
    #[should_panic(expected = "a must be less than or equal to b")]
    fn setup_wrong_a_and_b() {
        let rng = &mut rand::thread_rng();
        RPPublicParams::<Bls12>::setup(rng, 10, 2);
    }

    #[test]
    #[should_panic(expected = "log(log(b)) is zero")]
    fn setup_wrong_logb() {
        let rng = &mut rand::thread_rng();
        RPPublicParams::<Bls12>::setup(rng, -2, -1);
    }

    #[test]
    fn hash_works() {
        let rng = &mut rand::thread_rng();
        let D = G2::rand(rng);
        let D2 = G2::rand(rng);
        let params = setup(rng);
        let kp = BlindKeyPair::generate(rng, &params, 2);
        let m1 = Fr::rand(rng);
        let m2 = Fr::rand(rng);
        let sig = kp.sign(rng, &vec! {m1, m2});
        let state = kp.prove_commitment(rng, &params, &sig);
        let state1 = kp.prove_commitment(rng, &params, &sig);
        let state2 = kp.prove_commitment(rng, &params, &sig);
        let state3 = kp.prove_commitment(rng, &params, &sig);
        let state4 = kp.prove_commitment(rng, &params, &sig);
        let a = vec! {state, state1, state2};
        let a2 = vec! {state3, state4};
        assert_eq!(hash::<Bls12>(a.clone(), D.clone()).is_zero(), false);
        assert_ne!(hash::<Bls12>(a2.clone(), D.clone()), hash::<Bls12>(a.clone(), D.clone()));
        assert_ne!(hash::<Bls12>(a.clone(), D2.clone()), hash::<Bls12>(a.clone(), D.clone()));
        assert_ne!(hash::<Bls12>(a2.clone(), D2.clone()), hash::<Bls12>(a.clone(), D.clone()));
    }
}
