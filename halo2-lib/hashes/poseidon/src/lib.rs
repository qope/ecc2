// impl taken from https://github.com/scroll-tech/halo2-snark-aggregator/tree/main/halo2-snark-aggregator-api/src/hash

use ::poseidon::{SparseMDSMatrix, Spec, State};
use halo2_base::halo2_proofs::plonk::Error;
use halo2_base::{
    gates::GateInstructions,
    utils::ScalarField,
    AssignedValue, Context,
    QuantumCell::{Constant, Existing},
};

struct PoseidonState<'a, F: ScalarField, const T: usize, const RATE: usize> {
    s: [AssignedValue<'a, F>; T],
}

impl<'a, F: ScalarField, const T: usize, const RATE: usize> PoseidonState<'a, F, T, RATE> {
    fn x_power5_with_constant(
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        x: &AssignedValue<'a, F>,
        constant: &F,
    ) -> AssignedValue<'a, F> {
        let x2 = gate.mul(ctx, Existing(x), Existing(x));
        let x4 = gate.mul(ctx, Existing(&x2), Existing(&x2));
        gate.mul_add(ctx, Existing(x), Existing(&x4), Constant(*constant))
    }

    fn sbox_full(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        constants: &[F; T],
    ) {
        for (x, constant) in self.s.iter_mut().zip(constants.iter()) {
            *x = Self::x_power5_with_constant(ctx, gate, x, constant);
        }
    }

    fn sbox_part(&mut self, ctx: &mut Context<F>, gate: &impl GateInstructions<F>, constant: &F) {
        let x = &mut self.s[0];
        *x = Self::x_power5_with_constant(ctx, gate, x, constant);
    }

    fn absorb_with_pre_constants(
        &mut self,
        ctx: &mut Context<'a, F>,
        gate: &impl GateInstructions<F>,
        inputs: Vec<AssignedValue<'a, F>>,
        pre_constants: &[F; T],
    ) {
        assert!(inputs.len() < T);
        let offset = inputs.len() + 1;

        self.s[0] =
            gate.sum(ctx, inputs.iter().map(|a| Existing(a)).chain([Constant(pre_constants[0])]));

        for ((x, constant), input) in
            self.s.iter_mut().skip(1).zip(pre_constants.iter().skip(1)).zip(inputs.iter())
        {
            *x = gate.sum(ctx, [Existing(x), Existing(input), Constant(*constant)]);
        }

        for (i, (x, constant)) in
            self.s.iter_mut().skip(offset).zip(pre_constants.iter().skip(offset)).enumerate()
        {
            *x = gate.add(
                ctx,
                Existing(x),
                Constant(if i == 0 { F::one() + constant } else { *constant }),
            );
        }
    }

    fn apply_mds(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        mds: &[[F; T]; T],
    ) {
        let res = mds
            .iter()
            .map(|row| {
                gate.inner_product(
                    ctx,
                    self.s.iter().map(|x| Existing(x)),
                    row.iter().map(|c| Constant(*c)),
                )
            })
            .collect::<Vec<_>>();

        self.s = res.try_into().unwrap();
    }

    fn apply_sparse_mds(
        &mut self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        mds: &SparseMDSMatrix<F, T, RATE>,
    ) {
        let sum = gate.inner_product(
            ctx,
            self.s.iter().map(|x| Existing(x)),
            mds.row().iter().map(|c| Constant(*c)),
        );
        let mut res = vec![sum];

        for (e, x) in mds.col_hat().iter().zip(self.s.iter().skip(1)) {
            res.push(gate.mul_add(ctx, Existing(&self.s[0]), Constant(*e), Existing(x)));
        }

        for (x, new_x) in self.s.iter_mut().zip(res.into_iter()) {
            *x = new_x
        }
    }
}

pub struct PoseidonChip<'a, F: ScalarField, const T: usize, const RATE: usize> {
    init_state: [AssignedValue<'a, F>; T],
    state: PoseidonState<'a, F, T, RATE>,
    spec: Spec<F, T, RATE>,
    absorbing: Vec<AssignedValue<'a, F>>,
}

impl<'a, F: ScalarField, const T: usize, const RATE: usize> PoseidonChip<'a, F, T, RATE> {
    pub fn new(
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
        r_f: usize,
        r_p: usize,
    ) -> Result<Self, Error> {
        let init_state = State::<F, T>::default()
            .words()
            .into_iter()
            .map(|x| gate.load_constant(ctx, x))
            .collect::<Vec<AssignedValue<F>>>();
        Ok(Self {
            spec: Spec::new(r_f, r_p),
            init_state: init_state.clone().try_into().unwrap(),
            state: PoseidonState { s: init_state.try_into().unwrap() },
            absorbing: Vec::new(),
        })
    }

    pub fn clear(&mut self) {
        self.state = PoseidonState { s: self.init_state.clone() };
        self.absorbing.clear();
    }

    pub fn update(&mut self, elements: &[AssignedValue<'a, F>]) {
        self.absorbing.extend_from_slice(elements);
    }

    pub fn squeeze(
        &mut self,
        ctx: &mut Context<'a, F>,
        gate: &impl GateInstructions<F>,
    ) -> Result<AssignedValue<'a, F>, Error> {
        let mut input_elements = vec![];
        input_elements.append(&mut self.absorbing);

        let mut padding_offset = 0;

        for chunk in input_elements.chunks(RATE) {
            padding_offset = RATE - chunk.len();
            self.permutation(ctx, gate, chunk.to_vec());
        }

        if padding_offset == 0 {
            self.permutation(ctx, gate, vec![]);
        }

        Ok(self.state.s[1].clone())
    }

    fn permutation(
        &mut self,
        ctx: &mut Context<'a, F>,
        gate: &impl GateInstructions<F>,
        inputs: Vec<AssignedValue<'a, F>>,
    ) {
        let r_f = self.spec.r_f() / 2;
        let mds = &self.spec.mds_matrices().mds().rows();

        let constants = self.spec.constants().start();
        self.state.absorb_with_pre_constants(ctx, gate, inputs, &constants[0]);
        for constants in constants.iter().skip(1).take(r_f - 1) {
            self.state.sbox_full(ctx, gate, constants);
            self.state.apply_mds(ctx, gate, mds);
        }

        let pre_sparse_mds = &self.spec.mds_matrices().pre_sparse_mds().rows();
        self.state.sbox_full(ctx, gate, constants.last().unwrap());
        self.state.apply_mds(ctx, gate, pre_sparse_mds);

        let sparse_matrices = &self.spec.mds_matrices().sparse_matrices();
        let constants = &self.spec.constants().partial();
        for (constant, sparse_mds) in constants.iter().zip(sparse_matrices.iter()) {
            self.state.sbox_part(ctx, gate, constant);
            self.state.apply_sparse_mds(ctx, gate, sparse_mds);
        }

        let constants = &self.spec.constants().end();
        for constants in constants.iter() {
            self.state.sbox_full(ctx, gate, constants);
            self.state.apply_mds(ctx, gate, mds);
        }
        self.state.sbox_full(ctx, gate, &[F::zero(); T]);
        self.state.apply_mds(ctx, gate, mds);
    }
}
