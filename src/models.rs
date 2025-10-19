use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

// Competition data model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Competition {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    #[serde(with = "bson_datetime_as_rfc3339_string")]
    pub date: DateTime<Utc>,
    pub host: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(with = "option_bson_datetime_as_rfc3339_string")]
    pub signup_deadline: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_participants: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>, // e.g., "upcoming", "active", "completed", "cancelled"
}

// Helper module for serializing DateTime as RFC3339 string
mod bson_datetime_as_rfc3339_string {
    use chrono::{DateTime, Utc};
    
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&date.to_rfc3339())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom)
    }
}

// Helper module for serializing Option<DateTime> as RFC3339 string
mod option_bson_datetime_as_rfc3339_string {
    use chrono::{DateTime, Utc};
    
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(dt) => serializer.serialize_str(&dt.to_rfc3339()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        match opt {
            Some(s) => s.parse::<DateTime<Utc>>()
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

// Additional models that might be useful for a competition app
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Participant {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub email: String,
    pub competition_id: ObjectId,
    #[serde(with = "bson_datetime_as_rfc3339_string")]
    pub registration_date: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>, // e.g., "registered", "confirmed", "withdrawn"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompetitionResult {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub competition_id: ObjectId,
    pub participant_id: ObjectId,
    pub rank: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}