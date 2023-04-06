use halo2_base::halo2_proofs::{
    arithmetic::Field,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    halo2curves::{
        bn256::{Fq, Fq12, Fr, G1Affine, G2Affine},
        FieldExt,
    },
    plonk::{Circuit, ConstraintSystem, Error},
};
use halo2_ecc::{
    bn254::{pairing::PairingChip, Fp12Chip, FpChip},
    fields::{fp::FpStrategy, FieldChip},
};
use num_bigint::BigUint;
use num_traits::Num;

const K: usize = 16;

const NUM_ADVICE: usize = 100;
const NUM_LOOKUP_ADVICE: usize = 6;
const LIMB_BITS: usize = 90;
const LOOKUP_BITS: usize = 15;

#[derive(Clone, Default)]
struct MyCircuit;

impl Circuit<Fr> for MyCircuit {
    type Config = FpChip<Fr>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let p = BigUint::from_str_radix(&Fq::MODULUS[2..], 16).unwrap();
        let fp_chip = FpChip::configure(
            meta,
            FpStrategy::Simple,
            &[NUM_ADVICE],
            &[NUM_LOOKUP_ADVICE],
            1,
            LOOKUP_BITS,
            LIMB_BITS,
            3,
            p,
            0,
            K,
        );
        fp_chip
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        config.load_lookup_table(&mut layouter)?;
        let pairing_chip = PairingChip::construct(&config);
        let fp12chip = Fp12Chip::construct(&config);

        layouter.assign_region(
            || "my region",
            |region| {
                let ctx = &mut config.new_context(region);

                let mut rng = rand::thread_rng();
                let p = G1Affine::random(&mut rng);
                let q = G2Affine::random(&mut rng);
                dbg!(p); // Fq  2つからなる数
                dbg!(q); // Fq2 ２つからなる数
                let p_t = pairing_chip.load_private_g1(ctx, Value::known(p));
                let q_t = pairing_chip.load_private_g2(ctx, Value::known(q));

                let fq_point = pairing_chip.pairing(ctx, &q_t, &p_t);

                let one = fp12chip.load_constant(ctx, Fq12::one());

                let is_equal = fp12chip.is_equal(ctx, &fq_point, &one);
                // ctx.constrain_equal(&is_equal, Value::known(Fr::one()));
                dbg!(is_equal.value());
                Ok(())
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // use crate::utils::{gen_pk, gen_proof, gen_srs};

    use std::ops::*;

    use super::*;
    use ark_ec::pairing::Pairing;
    use ark_ff::One;
    use halo2_base::halo2_proofs::dev::MockProver;

    #[test]
    fn test_pairing() {
        let circuit = MyCircuit;
        MockProver::run(K as u32, &circuit, vec![])
            .unwrap()
            .assert_satisfied();

        // let params = gen_srs(K as u32);
        // let pk = gen_pk(&params, &circuit);
        // gen_proof(K, &params, &pk, circuit, vec![]);
    }

    #[test]
    fn test_ark_pairing() {
        use ark_bn254::{Bn254, G1Affine, G1Projective, G2Affine, G2Projective};
        // use ark_ec::PairingEngine;
        use ark_ff::UniformRand;

        let mut rng = rand::thread_rng();
        let a: G1Affine = G1Projective::rand(&mut rng).into();
        let b: G2Affine = G2Projective::rand(&mut rng).into();
        let c = Bn254::pairing(a, b);
        let c_prime = Bn254::pairing(a, b.neg());
        let d = c.0 * c_prime.0;

        dbg!(d.is_one());
    }
}
