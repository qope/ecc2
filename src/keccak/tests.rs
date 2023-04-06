#![allow(unused_imports)]
use super::*;
use crate::{
    rlc::RlcConfig,
    utils::{evm_verify, gen_evm_verifier},
};
use ark_std::{end_timer, start_timer};
use halo2_base::{
    gates::{
        flex_gate::{FlexGateConfig, GateStrategy},
        range::{RangeConfig, RangeStrategy},
    },
    halo2_proofs::{
        arithmetic::FieldExt,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        dev::MockProver,
        halo2curves::bn256::{Bn256, Fr, G1Affine},
        plonk::*,
        poly::commitment::{Params, ParamsProver},
        poly::kzg::{
            commitment::{KZGCommitmentScheme, ParamsKZG},
            multiopen::{ProverGWC, ProverSHPLONK, VerifierSHPLONK},
            strategy::SingleStrategy,
        },
        transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        transcript::{TranscriptReadBuffer, TranscriptWriterBuffer},
    },
    utils::{fe_to_biguint, fs::gen_srs, value_to_option, ScalarField},
    ContextParams, SKIP_FIRST_PASS,
};
use itertools::{assert_equal, Itertools};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::{
    env::{set_var, var},
    fs::File,
    io::{BufRead, BufReader, Write},
};
use zkevm_keccak::keccak_packed_multi::get_keccak_capacity;

// #[derive(Clone, Debug)]
// pub struct KeccakCircuit {
//     inputs: Vec<Vec<u8>>,
// }

// impl<F: Field> Circuit<F> for KeccakCircuit {
//     type Config = TestKeccakConfig<F>;
//     type FloorPlanner = SimpleFloorPlanner;

//     fn without_witnesses(&self) -> Self {
//         Self {
//             inputs: self
//                 .inputs
//                 .iter()
//                 .map(|input| vec![0u8; input.len()])
//                 .collect(),
//         }
//     }

//     fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
//         let num_advice: usize = var("NUM_ADVICE")
//             .unwrap_or_else(|_| "1".to_string())
//             .parse()
//             .unwrap();
//         let num_advice1: usize = var("NUM_ADVICE1")
//             .unwrap_or_else(|_| "1".to_string())
//             .parse()
//             .unwrap();
//         let degree: usize = var("KECCAK_DEGREE")
//             .unwrap_or_else(|_| "14".to_string())
//             .parse()
//             .unwrap();
//         let num_rlc_columns: usize = var("NUM_RLC")
//             .unwrap_or_else(|_| "1".to_string())
//             .parse()
//             .unwrap();
//         let mut range = RangeConfig::configure(
//             meta,
//             RangeStrategy::Vertical,
//             &[num_advice, num_advice1],
//             &[1, 1],
//             1,
//             8,
//             0,
//             degree,
//         );
//         let rlc = RlcConfig::configure(meta, num_rlc_columns, 1);
//         log::info!("unusable rows before keccak: {}", meta.minimum_rows());
//         let keccak = KeccakConfig::new(meta, rlc.gamma);
//         println!("unusable rows after keccak: {}", meta.minimum_rows());

//         let num_rows = (1 << degree) - meta.minimum_rows();
//         range.gate.max_rows = num_rows;

//         TestKeccakConfig {
//             range,
//             rlc,
//             keccak,
//             instance: meta.instance_column(),
//         }
//     }

//     fn synthesize(
//         &self,
//         config: Self::Config,
//         mut layouter: impl Layouter<F>,
//     ) -> Result<(), Error> {
//         let witness_time = start_timer!(|| "time witness gen");

//         config
//             .range
//             .load_lookup_table(&mut layouter)
//             .expect("load range lookup table");
//         config
//             .keccak
//             .load_aux_tables(&mut layouter)
//             .expect("load keccak lookup tables");
//         let mut first_pass = SKIP_FIRST_PASS;
//         layouter
//             .assign_region(
//                 || "keccak",
//                 |region| {
//                     if first_pass {
//                         first_pass = false;
//                         return Ok(());
//                     }

//                     let mut aux = Context::new(
//                         region,
//                         ContextParams {
//                             num_context_ids: 2,
//                             max_rows: config.range.gate.max_rows,
//                             fixed_columns: config.range.gate.constants.clone(),
//                         },
//                     );
//                     let ctx = &mut aux;

//                     let mut rlc_chip = RlcChip::new(config.rlc.clone(), Value::unknown());
//                     let mut keccak_chip = KeccakChip::new(config.keccak.clone());

//                     for (_idx, input) in self.inputs.iter().enumerate() {
//                         let bytes = input.to_vec();
//                         let bytes_assigned = config.range.gate.assign_witnesses(
//                             ctx,
//                             bytes.iter().map(|byte| Value::known(F::from(*byte as u64))),
//                         );
//                         // append some extra bytes to test variable length (don't do this for bench since it'll mess up the capacity)
//                         // let zero = config.range.gate.load_zero(ctx);
//                         // bytes_assigned.append(&mut vec![zero; _idx]);

//                         let len = config
//                             .range
//                             .gate
//                             .load_witness(ctx, Value::known(F::from(input.len() as u64)));

//                         let _hash = keccak_chip.keccak_var_len(
//                             ctx,
//                             &config.range,
//                             bytes_assigned,
//                             Some(bytes),
//                             len,
//                             0,
//                         );
//                     }
//                     keccak_chip.assign_phase0(&mut ctx.region);
//                     config.range.finalize(ctx);
//                     // END OF FIRST PHASE
//                     ctx.next_phase();

//                     // SECOND PHASE
//                     rlc_chip.get_challenge(ctx);
//                     keccak_chip.assign_phase1(ctx, &mut rlc_chip, &config.range);
//                     config.range.finalize(ctx);

//                     #[cfg(feature = "display")]
//                     {
//                         ctx.print_stats(&["Range", "RLC"]);
//                     }
//                     Ok(())
//                 },
//             )
//             .unwrap();
//         end_timer!(witness_time);

//         Ok(())
//     }
// }

// /// Cmdline: NUM_ADVICE=1 KECCAK_ROWS=25 KECCAK_DEGREE=14 RUST_LOG=info cargo test -- --nocapture test_keccak
// #[test]
// pub fn test_keccak() {
//     let _ = env_logger::builder().is_test(true).try_init();

//     let k: u32 = var("KECCAK_DEGREE")
//         .unwrap_or_else(|_| "14".to_string())
//         .parse()
//         .unwrap();
//     let inputs = vec![
//         vec![],
//         (0u8..1).collect::<Vec<_>>(),
//         (0u8..135).collect::<Vec<_>>(),
//         (0u8..136).collect::<Vec<_>>(),
//         (0u8..200).collect::<Vec<_>>(),
//     ];
//     let circuit = KeccakCircuit { inputs };

//     let prover = MockProver::<Fr>::run(k, &circuit, vec![]).unwrap();
//     prover.assert_satisfied();
// }

#[derive(Clone, Debug)]
pub struct TestKeccakConfig<F: Field> {
    range: RangeConfig<F>,
    rlc: RlcConfig<F>,
    keccak: KeccakConfig<F>,
    instance: Column<Instance>,
}

#[derive(Debug, Clone)]
struct MyKeccak;

impl Circuit<Fr> for MyKeccak {
    type Config = TestKeccakConfig<Fr>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let degree = 14;
        let mut range = RangeConfig::configure(
            meta,
            RangeStrategy::Vertical,
            &[1, 1],
            &[1, 1],
            1,
            8,
            0,
            degree,
        );
        let rlc = RlcConfig::configure(meta, 1, 1);
        let keccak = KeccakConfig::new(meta, rlc.gamma);
        let num_rows = (1 << degree) - meta.minimum_rows();
        range.gate.max_rows = num_rows;
        let instance = meta.instance_column();
        meta.enable_equality(instance);
        TestKeccakConfig {
            range,
            rlc,
            keccak,
            instance,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        config.range.load_lookup_table(&mut layouter)?;
        config.keccak.load_aux_tables(&mut layouter)?;

        let mut pi = vec![];
        layouter.assign_region(
            || "keccak",
            |region| {
                let ctx = &mut Context::new(
                    region,
                    ContextParams {
                        max_rows: config.range.gate.max_rows,
                        num_context_ids: 2,
                        fixed_columns: config.range.gate.constants.clone(),
                    },
                );

                let mut rlc_chip = RlcChip::new(config.rlc, Value::unknown());
                let mut keccak_chip = KeccakChip::new(config.keccak.clone());

                let input_bytes = b"hello".to_vec();

                let input_assigned = config.range.gate.assign_witnesses(
                    ctx,
                    input_bytes
                        .iter()
                        .map(|&x| Value::known(Fr::from(x as u64)))
                        .collect::<Vec<_>>(),
                );

                let hash_id = keccak_chip.keccak_fixed_len(
                    ctx,
                    &config.range.gate,
                    input_assigned,
                    Some(input_bytes.clone()),
                );

                dbg!(hash_id);
                let r = &keccak_chip.fixed_len_queries[hash_id];
                let output = hex::encode(r.output_bytes);
                dbg!(output);
                let expected_output = keccak256(input_bytes);
                let expected_output = hex::encode(expected_output);
                dbg!(expected_output);
                pi.push(*r.output_assigned[hash_id].cell());
                dbg!(&r.output_bytes[0]);

                keccak_chip.assign_phase0(&mut ctx.region);
                config.range.finalize(ctx);
                ctx.next_phase();
                rlc_chip.get_challenge(ctx);
                keccak_chip.assign_phase1(ctx, &mut rlc_chip, &config.range);
                config.range.finalize(ctx);
                Ok(())
            },
        )?;
        layouter.constrain_instance(pi[0], config.instance, 0);

        Ok(())
    }
}

// NUM_ADVICE=1 KECCAK_ROWS=25 KECCAK_DEGREE=14 RUST_LOG=info cargo test -- --nocapture test_keccak
#[test]
fn test_mykeccak() {
    use crate::utils::{gen_pk, gen_proof, gen_srs};

    let k: usize = 14;

    let circuit = MyKeccak;
    let instance = vec![vec![Fr::from(28)]];
    let prover = MockProver::<Fr>::run(k as u32, &circuit, instance.clone()).unwrap();
    prover.assert_satisfied();

    let params = gen_srs(k as u32);
    let pk = gen_pk(&params, &circuit);
    let proof = gen_proof(k, &params, &pk, circuit, instance.clone());

    let code = gen_evm_verifier(&params, pk.get_vk(), vec![1]);
    // evm_verify(code, instance, proof);
}
