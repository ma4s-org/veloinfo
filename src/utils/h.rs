use crate::db::edge::{EdgePoint, SourceOrTarget};
use crate::db::utils::distance_meters;

pub trait H: Send {
    fn get_cost(&self, edge: &EdgePoint) -> f64;
    fn get_max_point(&self) -> i64;

    fn h(&self, destination: &EdgePoint, goal: &EdgePoint) -> f64 {
        let (goal_lon, goal_lat) = if SourceOrTarget::Source == goal.direction {
            (goal.edge.lon1, goal.edge.lat1)
        } else {
            (goal.edge.lon2, goal.edge.lat2)
        };
        let (destination_lon, destination_lat) = if SourceOrTarget::Source == destination.direction
        {
            (destination.edge.lon1, destination.edge.lat1)
        } else {
            (destination.edge.lon2, destination.edge.lat2)
        };
        let distance = distance_meters(destination_lat, destination_lon, goal_lat, goal_lon);

        match distance {
            d if d > 100_000.0 => d * 3.2,
            d if d > 10_000.0 => d * 2.6,
            d => d * 1.41,
        }
    }
}

pub fn get_h_moyen() -> Box<dyn H> {
    Box::new(HMoyen {})
}

pub fn get_h_bigger_selection() -> Box<dyn H> {
    Box::new(HBiggerSelection {})
}

#[allow(dead_code)]
pub fn get_h_rapid() -> Box<dyn H> {
    Box::new(HRapid {})
}

pub struct HMoyen {}

impl H for HMoyen {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        // if the target is the source we are reverse of the edge
        if SourceOrTarget::Source == edge.direction
            && edge.edge.tags.get("oneway") == Some(&"yes".to_string())
            && edge.edge.tags.get("oneway:bicycle") != Some(&"no".to_string())
            && edge.edge.tags.get("cycleway:left:oneway") != Some(&"no".to_string())
            && edge.edge.tags.get("cycleway:right:oneway") != Some(&"no".to_string())
        {
            return 1. / 0.05;
        }

        let mut cost = if edge.edge.tags.get("bicycle") == Some(&"no".to_string())
            || edge.edge.tags.get("access") == Some(&"private".to_string())
            || edge.edge.tags.get("access") == Some(&"customers".to_string())
        {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()){
                1. / 0.3
            } else {
                1. / 0.05
            }
        } else if edge.edge.tags.get("highway") == Some(&"proposed".to_string())
            || edge.edge.tags.get("abandoned") == Some(&"yes".to_string())
            || edge.edge.tags.get("highway") == Some(&"motorway".to_string())
        {
            1. / 0.001
        } else if edge.edge.tags.get("informal") == Some(&"yes".to_string()) {
            1. / 0.05
        } else if edge.edge.tags.get("highway") == Some(&"steps".to_string()) {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.3
            } else {
                1. / 0.001
            }
        } else if edge.edge.tags.get("highway") == Some(&"path".to_string())
            && (edge.edge.tags.get("bicycle") == Some(&"dismount".to_string())
                || edge.edge.tags.get("bicycle") == Some(&"discouraged".to_string()))
        {
            1. / 0.1
        } else if edge.edge.tags.get("bicycle") == Some(&"discouraged".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("routing:bicycle") == Some(&"use_sidepath".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1.
            }
        } else if edge.edge.tags.get("cyclestreet") == Some(&"yes".to_string()) {
            1.
        } else if edge.edge.tags.get("cycleway") == Some(&"track".to_string()) {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1.
            }
        } else if edge.edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1.
            }
        } else if edge.edge.tags.get("cycleway:left") == Some(&"track".to_string())
            && (SourceOrTarget::Source == edge.direction
                || edge.edge.tags.get("cycleway:left:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1.
            }
        } else if edge.edge.tags.get("cycleway:right") == Some(&"track".to_string())
            && (SourceOrTarget::Target == edge.direction
                || edge.edge.tags.get("cycleway:right:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1.
            }
        } else if edge.edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1. / 0.9
            }
        } else if edge.edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1. / 0.9
            }
        } else if edge.edge.tags.get("cycleway:left") == Some(&"lane".to_string())
            && (SourceOrTarget::Source == edge.direction
                || edge.edge.tags.get("cycleway:left:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1. / 0.9
            }
        } else if edge.edge.tags.get("cycleway:right") == Some(&"lane".to_string())
            && (SourceOrTarget::Target == edge.direction
                || edge.edge.tags.get("cycleway:right:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string()) {
                1. / 0.6
            } else {
                1. / 0.9
            }
        } else if edge.edge.tags.get("higway") == Some(&"footway".to_string())
            && edge.edge.tags.get("footway") == Some(&"crossing".to_string())
        {
            1. / 0.7
        } else if edge.edge.tags.get("highway") == Some(&"footway".to_string()) {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string())
                || edge.edge.tags.get("bicycle") == Some(&"designated".to_string())
            {
                if edge.edge.tags.get("footway") == Some(&"sidewalk".to_string()) {
                    1. / 0.4
                } else {
                    1. / 0.9
                }
            } else if edge.edge.tags.get("bicycle") == Some(&"dismount".to_string()) {
                1. / 0.3
            } else {
                1. / 0.1
            }
        } else if edge.edge.tags.get("cycleway") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway") == Some(&"share_busway".to_string())
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string())
                || edge.edge.tags.get("surface") == Some(&"sett".to_string())
            {
                1. / 0.5
            } else {
                1. / 0.7
            }
        } else if edge.edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:both") == Some(&"share_busway".to_string())
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string())
                || edge.edge.tags.get("surface") == Some(&"sett".to_string())
            {
                1. / 0.5
            } else {
                1. / 0.7
            }
        } else if edge.edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:left") == Some(&"share_busway".to_string())
                && (SourceOrTarget::Source == edge.direction
                    || edge.edge.tags.get("cycleway:left:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string())
                || edge.edge.tags.get("surface") == Some(&"sett".to_string())
            {
                1. / 0.5
            } else {
                1. / 0.7
            }
        } else if edge.edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:right") == Some(&"share_busway".to_string())
                && (SourceOrTarget::Target == edge.direction
                    || edge.edge.tags.get("cycleway:right:oneway") == Some(&"no".to_string()))
        {
            if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
                1. / 0.6
            } else if edge.edge.tags.get("smoothness") == Some(&"bad".to_string())
                || edge.edge.tags.get("surface") == Some(&"sett".to_string())
            {
                1. / 0.5
            } else {
                1. / 0.7
            }
        } else if edge.edge.tags.get("highway") == Some(&"residential".to_string()) {
            if edge.edge.tags.get("surface") == Some(&"sett".to_string())
                || edge.edge.tags.get("surface") == Some(&"cobblestone".to_string())
            {
                1. / 0.4
            } else if edge.edge.tags.get("bicycle") == Some(&"yes".to_string())
                || edge.edge.tags.get("bicycle") == Some(&"designated".to_string())
            {
                1. / 0.85
            } else {
                1. / 0.6
            }
        } else if edge.edge.tags.get("highway") == Some(&"service".to_string()) {
            if edge.edge.tags.get("surface") == Some(&"chipseal".to_string()) {
                1. / 0.5
            } else {
                1. / 0.6
            }
        } else if edge.edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.5
        } else if edge.edge.tags.get("highway") == Some(&"tertiary".to_string()) {
            if edge.edge.tags.get("surface") == Some(&"sett".to_string())
                || edge.edge.tags.get("surface") == Some(&"cobblestone".to_string())
            {
                1. / 0.3
            } else if edge.edge.in_bicycle_route {
                1. / 0.6
            } else {
                1. / 0.5
            }
        } else if edge.edge.tags.get("higway") == Some(&"tertiary_link".to_string()) {
            1. / 0.5
        } else if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
            1. / 0.5
        } else if edge.edge.tags.get("highway") == Some(&"secondary".to_string()) {
            if edge.edge.tags.get("surface") == Some(&"sett".to_string())
                || edge.edge.tags.get("surface") == Some(&"cobblestone".to_string())
            {
                1. / 0.3
            } else if edge.edge.in_bicycle_route {
                1. / 0.6
            } else {
                1. / 0.4
            }
        } else if edge.edge.in_bicycle_route {
            1. / 0.6
        } else if edge.edge.tags.get("highway") == Some(&"secondary_link".to_string()) {
            1. / 0.4
        } else if edge.edge.tags.get("higway") == Some(&"primary".to_string()) {
            1. / 0.3
        } else if edge.edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.7
        } else if edge.edge.tags.get("higway") == Some(&"trunk".to_string()) {
            1. / 0.3
        } else if edge.edge.tags.get("higway").is_some() {
            1. / 0.3
        } else if edge.edge.tags.get("higway") == Some(&"footway".to_string()) {
            1. / 0.1
        } else {
            1. / 0.05
        };

        if edge.edge.road_work {
            cost = cost * 3.;
        }
        let score = match edge.edge.score {
            Some(score) => {
                if score == -1.0 {
                    1.
                } else if score == 0. {
                    0.01
                } else {
                    score
                }
            }
            None => 1.,
        };
        cost / score
    }

    fn get_max_point(&self) -> i64 {
        i64::MAX
    }
}

pub struct HBiggerSelection {}

impl H for HBiggerSelection {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        let cost = if edge.edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            1.
        } else if edge.edge.tags.get("cyclestreet") == Some(&"yes".to_string()) {
            1.
        } else if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
            1.
        } else if edge.edge.tags.get("cycleway") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:left") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:right") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:left") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:right") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway") == Some(&"share_busway".to_string())
        {
            1. / 0.7
        } else if edge.edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:both") == Some(&"share_busway".to_string())
        {
            1. / 0.7
        } else if edge.edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.7
        } else if edge.edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.7
        } else if edge.edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:left") == Some(&"share_busway".to_string())
        {
            1. / 0.7
        } else if edge.edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            || edge.edge.tags.get("cycleway:right") == Some(&"share_busway".to_string())
        {
            1. / 0.7
        } else {
            1. / 0.1
        };

        cost
    }

    fn get_max_point(&self) -> i64 {
        10_000
    }
}

pub struct HRapid {}

impl H for HRapid {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        if SourceOrTarget::Source == edge.direction
            && edge.edge.tags.get("oneway") == Some(&"yes".to_string())
            && (!edge.edge.tags.get("cycleway:both").is_some()
                || edge.edge.tags.get("cycleway:both") == Some(&"no".to_string()))
            && (!edge.edge.tags.get("cycleway:left").is_some()
                || edge.edge.tags.get("cycleway:left") == Some(&"no".to_string()))
            && edge.edge.tags.get("oneway:bicycle") != Some(&"no".to_string())
        {
            return 1. / 0.05;
        }

        let mut cost = if edge.edge.tags.get("bicycle") == Some(&"no".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("higway") == Some(&"proposed".to_string()) {
            1. / 0.05
        } else if edge.edge.tags.get("informal") == Some(&"yes".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("highway") == Some(&"steps".to_string()) {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.3
            } else {
                1. / 0.1
            }
        } else if edge.edge.tags.get("highway") == Some(&"path".to_string())
            && (edge.edge.tags.get("bicycle") == Some(&"dismount".to_string())
                || edge.edge.tags.get("bicycle") == Some(&"discouraged".to_string()))
        {
            1. / 0.05
        } else if edge.edge.tags.get("bicycle") == Some(&"discouraged".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("routing:bicycle") == Some(&"use_sidepath".to_string()) {
            1. / 0.1
        } else if edge.edge.tags.get("bicycle") == Some(&"dismount".to_string()) {
            1. / 0.2
        } else if edge.edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            1.
        } else if edge.edge.tags.get("cyclestreet") == Some(&"yes".to_string()) {
            1.
        } else if edge.edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
            1.
        } else if edge.edge.tags.get("cycleway") == Some(&"track".to_string()) {
            1. / 0.95
        } else if edge.edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            1. / 0.95
        } else if edge.edge.tags.get("cycleway:left") == Some(&"track".to_string())
            && SourceOrTarget::Source == edge.direction
        {
            1. / 0.95
        } else if edge.edge.tags.get("cycleway:right") == Some(&"track".to_string())
            && SourceOrTarget::Target == edge.direction
        {
            1. / 0.95
        } else if edge.edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:left") == Some(&"lane".to_string())
            && SourceOrTarget::Source == edge.direction
        {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway:right") == Some(&"lane".to_string())
            && SourceOrTarget::Target == edge.direction
        {
            1. / 0.9
        } else if edge.edge.tags.get("footway") == Some(&"path".to_string()) {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.9
            } else {
                1. / 0.1
            }
        } else if edge.edge.tags.get("highway") == Some(&"footway".to_string())
            || edge.edge.tags.get("highway") == Some(&"pedestrian".to_string())
        {
            1. / 0.9
        } else if edge.edge.tags.get("cycleway") == Some(&"shared_lane".to_string()) {
            1. / 0.85
        } else if edge.edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string()) {
            1. / 0.85
        } else if edge.edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.85
        } else if edge.edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            && SourceOrTarget::Source == edge.direction
        {
            1. / 0.85
        } else if edge.edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            && SourceOrTarget::Target == edge.direction
        {
            1. / 0.85
        } else if edge.edge.tags.get("highway") == Some(&"residential".to_string()) {
            if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.85
            } else {
                1. / 0.8
            }
        } else if edge.edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("highway") == Some(&"tertiary".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("higway") == Some(&"tertiary_link".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("higway") == Some(&"footway".to_string())
            && edge.edge.tags.get("footway") == Some(&"crossing".to_string())
        {
            1. / 0.8
        } else if edge.edge.tags.get("bicycle") == Some(&"yes".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway") == Some(&"separate".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:both") == Some(&"separate".to_string()) {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:left") == Some(&"separate".to_string())
            && SourceOrTarget::Source == edge.direction
        {
            1. / 0.8
        } else if edge.edge.tags.get("cycleway:right") == Some(&"separate".to_string())
            && SourceOrTarget::Target == edge.direction
        {
            1. / 0.8
        } else if edge.edge.tags.get("highway") == Some(&"secondary".to_string()) {
            1. / 0.75
        } else if edge.edge.tags.get("highway") == Some(&"secondary_link".to_string()) {
            1. / 0.75
        } else if edge.edge.tags.get("higway") == Some(&"primary".to_string()) {
            1. / 0.75
        } else if edge.edge.tags.get("higway") == Some(&"trunk".to_string()) {
            1. / 0.75
        } else if edge.edge.tags.get("higway").is_some() {
            1. / 0.5
        } else if edge.edge.tags.get("higway") == Some(&"footway".to_string()) {
            1. / 0.1
        } else {
            1. / 0.1
        };

        if edge.edge.road_work {
            cost = 1. / 0.5;
        }

        let score = 1.
            / match edge.edge.score {
                Some(score) => {
                    if score == 0. {
                        0.01
                    } else if score == -1.0 {
                        1.0
                    } else {
                        score
                    }
                }
                None => 1.,
            };
        cost * score
    }

    fn get_max_point(&self) -> i64 {
        1000000000000000000
    }
}
