use std::marker::PhantomData;

use halo2_base::{
    gates::{
        flex_gate::{FlexGateConfig, GateStrategy},
        GateInstructions,
    },
    halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::bn256::Fr,
        plonk::{
            Advice, Challenge, Circuit, Column, ConstraintSystem, Error, FirstPhase, Instance,
        },
    },
    utils::ScalarField,
    Context, ContextParams, QuantumCell,
};

const K: usize = 19;

#[derive(Clone)]
struct MyConfig<F: ScalarField> {
    flex_gate: FlexGateConfig<F>,
    a: Column<Advice>,
    c: Challenge,
    instance: Column<Instance>,
}

#[derive(Clone, Default)]
struct MyCircuit<F> {
    _marker: PhantomData<F>,
}

impl Circuit<Fr> for MyCircuit<Fr> {
    type Config = MyConfig<Fr>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<Fr>) -> Self::Config {
        let flex_gate = FlexGateConfig::configure(meta, GateStrategy::Vertical, &[1], 1, 0, K);
        let instance = meta.instance_column();
        let a = meta.advice_column();
        meta.enable_equality(a);
        meta.enable_equality(instance);

        let c = meta.challenge_usable_after(FirstPhase);
        Self::Config {
            flex_gate,
            a,
            c,
            instance,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fr>,
    ) -> Result<(), Error> {
        let instance_col = config.clone().instance;
        let flex_gate = config.flex_gate;
        let c = layouter.get_challenge(config.c);
        layouter.assign_region(
            || "my region",
            |region| {
                let ctx = &mut Context::new(
                    region,
                    ContextParams {
                        max_rows: K,
                        num_context_ids: 1,
                        fixed_columns: flex_gate.constants.clone(),
                    },
                );
                let c = flex_gate.load_witness(ctx, c);
                let a = flex_gate.add(
                    ctx,
                    QuantumCell::Existing(&c),
                    QuantumCell::Constant(Fr::one()),
                );
                dbg!(&a);

                ctx.next_phase();

                dbg!(&c);

                Ok(())
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::halo2_proofs::dev::MockProver;

    #[test]
    fn test_mycircuit() {
        let circuit = MyCircuit::<Fr> {
            _marker: PhantomData,
        };
        MockProver::run(K as u32, &circuit, vec![vec![]])
            .unwrap()
            .assert_satisfied();
    }
}
