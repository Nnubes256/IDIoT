use std::collections::HashMap;

use diot_core::device::Measurement;
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

use crate::{
    hardware::{FullActuatorData, FullSensorData},
    system::peerid_opt_parse,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ConditionOp {
    Any,
    Equal { value: Measurement },
    GreaterThan { value: Measurement },
    LessThan { value: Measurement },
    GreaterOrEqualThan { value: Measurement },
    LessOrEqualThan { value: Measurement },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UniversalSensorIdentifier {
    #[serde(default, with = "peerid_opt_parse")]
    node: Option<PeerId>,
    device: String,
    sensor_name: String,
}

impl UniversalSensorIdentifier {
    pub fn from_local(data: FullSensorData) -> Self {
        Self {
            node: None,
            device: data.device,
            sensor_name: data.sensor_name,
        }
    }

    pub fn from_remote(node: PeerId, data: FullSensorData) -> Self {
        Self {
            node: Some(node),
            device: data.device,
            sensor_name: data.sensor_name,
        }
    }

    pub fn corresponds_with(&self, data: &FullSensorData) -> bool {
        data.device == self.device && data.sensor_name == self.sensor_name
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(default, with = "peerid_opt_parse")]
    pub node: Option<PeerId>,
    #[serde(flatten)]
    pub actuator: FullActuatorData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    sensor: UniversalSensorIdentifier,
    on: ConditionOp,
    then: Action,
}

pub struct ControlLayer {
    rule_triggers: HashMap<UniversalSensorIdentifier, Vec<usize>>,
    rules: Vec<Rule>,
}

impl ControlLayer {
    pub fn from_ruleset(rules: Vec<Rule>) -> Self {
        info!("Loading {} rules", rules.len());

        let mut rule_triggers = HashMap::new();
        for (i, rule) in rules.iter().enumerate() {
            rule_triggers
                .entry(rule.sensor.clone())
                .and_modify(|v: &mut Vec<_>| v.push(i))
                .or_insert_with(|| vec![i]);
        }

        Self {
            rule_triggers,
            rules,
        }
    }

    fn evaluate_rule(rule: &Rule, input: &FullSensorData, check_source: bool) -> bool {
        if check_source && !rule.sensor.corresponds_with(input) {
            return false;
        }

        match &rule.on {
            ConditionOp::Any => true,
            ConditionOp::Equal { value } => input.value.eq(value),
            ConditionOp::GreaterThan { value } => input.value.gt(value).unwrap_or(false),
            ConditionOp::LessThan { value } => input.value.lt(value).unwrap_or(false),
            ConditionOp::GreaterOrEqualThan { value } => input.value.geq(value).unwrap_or(false),
            ConditionOp::LessOrEqualThan { value } => input.value.leq(value).unwrap_or(false),
        }
    }

    pub fn trigger_local(&mut self, sensor: &FullSensorData) -> Option<Vec<Action>> {
        let sensor_id = UniversalSensorIdentifier::from_local(sensor.clone());

        if let Some(rules) = self.rule_triggers.get(&sensor_id) {
            let mut actions = Vec::new();

            for rule_idx in rules {
                let rule = self.rules.get(*rule_idx).expect("a rule to be there");

                if Self::evaluate_rule(rule, sensor, false) {
                    actions.push(rule.then.clone());
                }
            }

            Some(actions)
        } else {
            None
        }
    }

    pub fn trigger_remote(&mut self, peer: PeerId, sensor: &FullSensorData) -> Option<Vec<Action>> {
        let sensor_id = UniversalSensorIdentifier::from_remote(peer, sensor.clone());

        if let Some(rules) = self.rule_triggers.get(&sensor_id) {
            let mut actions = Vec::new();

            for rule_idx in rules {
                let rule = self.rules.get(*rule_idx).expect("a rule to be there");

                if Self::evaluate_rule(rule, sensor, false) {
                    info!("Sensor event matches local rule {}, triggering", rule_idx);
                    actions.push(rule.then.clone());
                }
            }

            Some(actions)
        } else {
            None
        }
    }
}
