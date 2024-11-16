use crate::db::edge::Edge;
use crate::db::utils::distance_meters;

pub trait H: Send {
    fn get_cost(&self, edge: &Edge, target: i64) -> f64;
    fn h(&self, destination: &Edge, destination_id: i64, goal: &Edge, gaol_id: i64) -> f64;
    fn get_max_point(&self) -> i64;
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
    fn get_cost(&self, edge: &Edge, target: i64) -> f64 {
        // if the target is the source we are reverse of the edge
        if target == edge.source
            && edge.tags.get("oneway") == Some(&"yes".to_string())
            && edge.tags.get("oneway:bicycle") != Some(&"no".to_string())
            && edge.tags.get("cycleway:left:oneway") != Some(&"no".to_string())
        {
            return 1. / 0.05;
        }

        let mut cost = if edge.tags.get("bicycle") == Some(&"no".to_string()) {
            1. / 0.1
        } else if edge.tags.get("higway") == Some(&"proposed".to_string()) {
            1. / 0.05
        } else if edge.tags.get("informal") == Some(&"yes".to_string()) {
            1. / 0.1
        } else if edge.tags.get("highway") == Some(&"steps".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.3
            } else {
                1. / 0.1
            }
        } else if edge.tags.get("highway") == Some(&"path".to_string())
            && (edge.tags.get("bicycle") == Some(&"dismount".to_string())
                || edge.tags.get("bicycle") == Some(&"discouraged".to_string()))
        {
            1. / 0.05
        } else if edge.tags.get("bicycle") == Some(&"discouraged".to_string()) {
            1. / 0.1
        } else if edge.tags.get("routing:bicycle") == Some(&"use_sidepath".to_string()) {
            1. / 0.1
        } else if edge.tags.get("bicycle") == Some(&"dismount".to_string()) {
            1. / 0.2
        } else if edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:left") == Some(&"track".to_string())
            && target == edge.source
        {
            1. / 0.9
        } else if edge.tags.get("cycleway:right") == Some(&"track".to_string())
            && target == edge.target
        {
            1. / 0.9
        } else if edge.tags.get("higway") == Some(&"footway".to_string())
            && edge.tags.get("footway") == Some(&"crossing".to_string())
        {
            1. / 0.8
        } else if edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:left") == Some(&"lane".to_string())
            && target == edge.source
        {
            1. / 0.8
        } else if edge.tags.get("cycleway:right") == Some(&"lane".to_string())
            && target == edge.target
        {
            1. / 0.8
        } else if edge.tags.get("footway") == Some(&"path".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.8
            } else {
                1. / 0.2
            }
        } else if edge.tags.get("highway") == Some(&"footway".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.8
            } else {
                1. / 0.2
            }
        } else if edge.tags.get("cycleway") == Some(&"shared_lane".to_string()) {
            1. / 0.7
        } else if edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string()) {
            1. / 0.7
        } else if edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            && target == edge.source
        {
            1. / 0.7
        } else if edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            && target == edge.target
        {
            1. / 0.7
        } else if edge.tags.get("highway") == Some(&"residential".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.7
            } else {
                1. / 0.6
            }
        } else if edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.5
        } else if edge.tags.get("highway") == Some(&"tertiary".to_string()) {
            1. / 0.5
        } else if edge.tags.get("higway") == Some(&"tertiary_link".to_string()) {
            1. / 0.5
        } else if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
            1. / 0.5
        } else if edge.tags.get("highway") == Some(&"service".to_string()) {
            1. / 0.5
        } else if edge.tags.get("cycleway") == Some(&"separate".to_string()) {
            1. / 0.4
        } else if edge.tags.get("cycleway:both") == Some(&"separate".to_string()) {
            1. / 0.4
        } else if edge.tags.get("cycleway:left") == Some(&"separate".to_string())
            && target == edge.source
        {
            1. / 0.4
        } else if edge.tags.get("cycleway:right") == Some(&"separate".to_string())
            && target == edge.target
        {
            1. / 0.4
        } else if edge.tags.get("highway") == Some(&"secondary".to_string()) {
            1. / 0.35
        } else if edge.tags.get("highway") == Some(&"secondary_link".to_string()) {
            1. / 0.35
        } else if edge.tags.get("higway") == Some(&"primary".to_string()) {
            1. / 0.3
        } else if edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.7
        } else if edge.tags.get("higway") == Some(&"trunk".to_string()) {
            1. / 0.3
        } else if edge.tags.get("higway").is_some() {
            1. / 0.3
        } else if edge.tags.get("higway") == Some(&"footway".to_string()) {
            1. / 0.2
        } else {
            1. / 0.05
        };

        if edge.road_work {
            cost = 1. / 0.5;
        }
        let score = match edge.score {
            Some(score) => {
                if score <= 0. {
                    0.01
                } else {
                    score
                }
            }
            None => 1.,
        };
        cost / score
    }

    fn h(&self, destination: &Edge, destination_id: i64, goal: &Edge, gaol_id: i64) -> f64 {
        let (goal_lon, goal_lat) = if gaol_id == goal.source {
            (goal.lon1, goal.lat1)
        } else {
            (goal.lon2, goal.lat2)
        };
        let (destination_lon, destination_lat) = if destination_id == destination.source {
            (destination.lon1, destination.lat1)
        } else {
            (destination.lon2, destination.lat2)
        };
        let distance = distance_meters(destination_lat, destination_lon, goal_lat, goal_lon);

        distance * self.get_cost(destination, destination_id)
    }

    fn get_max_point(&self) -> i64 {
        1000000000000000000
    }
}

pub struct HBiggerSelection {}

impl H for HBiggerSelection {
    fn get_cost(&self, edge: &Edge, target: i64) -> f64 {
        let cost = if edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:left") == Some(&"track".to_string())
            && target == edge.source
        {
            1. / 0.9
        } else if edge.tags.get("cycleway:right") == Some(&"track".to_string())
            && target == edge.target
        {
            1. / 0.9
        } else if edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:left") == Some(&"lane".to_string())
            && target == edge.source
        {
            1. / 0.8
        } else if edge.tags.get("cycleway:right") == Some(&"lane".to_string())
            && target == edge.target
        {
            1. / 0.8
        } else if edge.tags.get("cycleway") == Some(&"shared_lane".to_string()) {
            1. / 0.7
        } else if edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string()) {
            1. / 0.7
        } else if edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.7
        } else if edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.7
        } else if edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            && target == edge.source
        {
            1. / 0.7
        } else if edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            && target == edge.target
        {
            1. / 0.7
        } else {
            1. / 0.1
        };

        cost
    }

    fn h(&self, destination: &Edge, destination_id: i64, goal: &Edge, gaol_id: i64) -> f64 {
        let (goal_lon, goal_lat) = if gaol_id == goal.source {
            (goal.lon1, goal.lat1)
        } else {
            (goal.lon2, goal.lat2)
        };
        let (destination_lon, destination_lat) = if destination_id == destination.source {
            (destination.lon1, destination.lat1)
        } else {
            (destination.lon2, destination.lat2)
        };
        let distance = distance_meters(destination_lat, destination_lon, goal_lat, goal_lon);

        distance * self.get_cost(destination, destination_id)
    }

    fn get_max_point(&self) -> i64 {
        10_000
    }
}

pub struct HRapid {}

impl H for HRapid {
    fn get_cost(&self, edge: &Edge, target: i64) -> f64 {
        if target == edge.source
            && edge.tags.get("oneway") == Some(&"yes".to_string())
            && (!edge.tags.get("cycleway:both").is_some()
                || edge.tags.get("cycleway:both") == Some(&"no".to_string()))
            && (!edge.tags.get("cycleway:left").is_some()
                || edge.tags.get("cycleway:left") == Some(&"no".to_string()))
            && edge.tags.get("oneway:bicycle") != Some(&"no".to_string())
        {
            return 1. / 0.05;
        }

        let mut cost = if edge.tags.get("bicycle") == Some(&"no".to_string()) {
            1. / 0.1
        } else if edge.tags.get("higway") == Some(&"proposed".to_string()) {
            1. / 0.05
        } else if edge.tags.get("informal") == Some(&"yes".to_string()) {
            1. / 0.1
        } else if edge.tags.get("highway") == Some(&"steps".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.3
            } else {
                1. / 0.1
            }
        } else if edge.tags.get("highway") == Some(&"path".to_string())
            && (edge.tags.get("bicycle") == Some(&"dismount".to_string())
                || edge.tags.get("bicycle") == Some(&"discouraged".to_string()))
        {
            1. / 0.05
        } else if edge.tags.get("bicycle") == Some(&"discouraged".to_string()) {
            1. / 0.1
        } else if edge.tags.get("routing:bicycle") == Some(&"use_sidepath".to_string()) {
            1. / 0.1
        } else if edge.tags.get("bicycle") == Some(&"dismount".to_string()) {
            1. / 0.2
        } else if edge.tags.get("highway") == Some(&"cycleway".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"crossing".to_string()) {
            1.
        } else if edge.tags.get("cycleway") == Some(&"track".to_string()) {
            1. / 0.95
        } else if edge.tags.get("cycleway:both") == Some(&"track".to_string()) {
            1. / 0.95
        } else if edge.tags.get("cycleway:left") == Some(&"track".to_string())
            && target == edge.source
        {
            1. / 0.95
        } else if edge.tags.get("cycleway:right") == Some(&"track".to_string())
            && target == edge.target
        {
            1. / 0.95
        } else if edge.tags.get("cycleway") == Some(&"lane".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:both") == Some(&"lane".to_string()) {
            1. / 0.9
        } else if edge.tags.get("cycleway:left") == Some(&"lane".to_string())
            && target == edge.source
        {
            1. / 0.9
        } else if edge.tags.get("cycleway:right") == Some(&"lane".to_string())
            && target == edge.target
        {
            1. / 0.9
        } else if edge.tags.get("footway") == Some(&"path".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.9
            } else {
                1. / 0.2
            }
        } else if edge.tags.get("highway") == Some(&"footway".to_string())
            || edge.tags.get("highway") == Some(&"pedestrian".to_string())
        {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.9
            } else {
                1. / 0.2
            }
        } else if edge.tags.get("cycleway") == Some(&"shared_lane".to_string()) {
            1. / 0.85
        } else if edge.tags.get("cycleway:both") == Some(&"shared_lane".to_string()) {
            1. / 0.85
        } else if edge.tags.get("bicycle") == Some(&"designated".to_string()) {
            1. / 0.85
        } else if edge.tags.get("cycleway:left") == Some(&"shared_lane".to_string())
            && target == edge.source
        {
            1. / 0.85
        } else if edge.tags.get("cycleway:right") == Some(&"shared_lane".to_string())
            && target == edge.target
        {
            1. / 0.85
        } else if edge.tags.get("highway") == Some(&"residential".to_string()) {
            if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
                1. / 0.85
            } else {
                1. / 0.8
            }
        } else if edge.tags.get("highway") == Some(&"unclassified".to_string()) {
            1. / 0.8
        } else if edge.tags.get("highway") == Some(&"tertiary".to_string()) {
            1. / 0.8
        } else if edge.tags.get("higway") == Some(&"tertiary_link".to_string()) {
            1. / 0.8
        } else if edge.tags.get("higway") == Some(&"footway".to_string())
            && edge.tags.get("footway") == Some(&"crossing".to_string())
        {
            1. / 0.8
        } else if edge.tags.get("bicycle") == Some(&"yes".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway") == Some(&"separate".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:both") == Some(&"separate".to_string()) {
            1. / 0.8
        } else if edge.tags.get("cycleway:left") == Some(&"separate".to_string())
            && target == edge.source
        {
            1. / 0.8
        } else if edge.tags.get("cycleway:right") == Some(&"separate".to_string())
            && target == edge.target
        {
            1. / 0.8
        } else if edge.tags.get("highway") == Some(&"secondary".to_string()) {
            1. / 0.75
        } else if edge.tags.get("highway") == Some(&"secondary_link".to_string()) {
            1. / 0.75
        } else if edge.tags.get("higway") == Some(&"primary".to_string()) {
            1. / 0.75
        } else if edge.tags.get("higway") == Some(&"trunk".to_string()) {
            1. / 0.75
        } else if edge.tags.get("highway") == Some(&"service".to_string()) {
            1. / 0.7
        } else if edge.tags.get("higway").is_some() {
            1. / 0.5
        } else if edge.tags.get("higway") == Some(&"footway".to_string()) {
            1. / 0.2
        } else {
            1. / 0.1
        };

        if edge.road_work {
            cost = 1. / 0.5;
        }

        let score = 1.
            / match edge.score {
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

    fn h(&self, destination: &Edge, destination_id: i64, goal: &Edge, gaol_id: i64) -> f64 {
        let (goal_lon, goal_lat) = if gaol_id == goal.source {
            (goal.lon1, goal.lat1)
        } else {
            (goal.lon2, goal.lat2)
        };
        let (destination_lon, destination_lat) = if destination_id == destination.source {
            (destination.lon1, destination.lat1)
        } else {
            (destination.lon2, destination.lat2)
        };
        let distance = distance_meters(destination_lat, destination_lon, goal_lat, goal_lon);

        distance * self.get_cost(destination, destination_id)
    }

    fn get_max_point(&self) -> i64 {
        1000000000000000000
    }
}
