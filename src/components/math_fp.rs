use js_sys::Math;
use serde::Serialize;

#[derive(Serialize)]
pub struct MathFp {
    pub probes: Vec<Probe>,
    pub hash: String,
}

#[derive(Serialize)]
pub struct Probe {
    pub name: &'static str,
    pub value: f64,
}

pub fn collect() -> MathFp {
    let probes = vec![
        Probe {
            name: "acos_0123",
            value: Math::acos(0.123_124_234_234_234_24),
        },
        Probe {
            name: "acosh_1e308",
            value: Math::acosh(1e308),
        },
        Probe {
            name: "acosh_1e154",
            value: Math::acosh(1e154),
        },
        Probe {
            name: "asin_0123",
            value: Math::asin(0.123_124_234_234_234_24),
        },
        Probe {
            name: "asinh_1e300",
            value: Math::asinh(1e300),
        },
        Probe {
            name: "atanh_05",
            value: Math::atanh(0.5),
        },
        Probe {
            name: "cbrt_100",
            value: Math::cbrt(100.0),
        },
        Probe {
            name: "cos_13e",
            value: Math::cos(13.0 * std::f64::consts::E),
        },
        Probe {
            name: "cosh_1",
            value: Math::cosh(1.0),
        },
        Probe {
            name: "expm1_1",
            value: Math::expm1(1.0),
        },
        Probe {
            name: "exp_1",
            value: Math::exp(1.0),
        },
        Probe {
            name: "log_d",
            value: Math::log(std::f64::consts::E),
        },
        Probe {
            name: "log1p_99",
            value: Math::log1p(99.0),
        },
        Probe {
            name: "pow_pi_neg100",
            value: Math::pow(std::f64::consts::PI, -100.0),
        },
        Probe {
            name: "sin_39e",
            value: Math::sin(39.0 * std::f64::consts::E),
        },
        Probe {
            name: "sinh_1",
            value: Math::sinh(1.0),
        },
        Probe {
            name: "tan_3",
            value: Math::tan(3.0),
        },
        Probe {
            name: "tan_neg_1e308",
            value: Math::tan(-1e308),
        },
        Probe {
            name: "tanh_2",
            value: Math::tanh(2.0),
        },
    ];

    // Hash IEEE 754 byte representation — captures last-ULP differences across engines.
    let mut buf: Vec<u8> = Vec::with_capacity(probes.len() * 24);
    for p in &probes {
        buf.extend_from_slice(p.name.as_bytes());
        buf.push(b':');
        buf.extend_from_slice(&p.value.to_le_bytes());
        buf.push(b'|');
    }
    let hash = crate::hash::hash_bytes(&buf);

    MathFp { probes, hash }
}
