use sqlx::Postgres;

#[derive(sqlx::FromRow, Debug)]
pub struct SearchResultDB {
    pub name: String,
    pub lng: f64,
    pub lat: f64,
}

pub async fn get(
    request: &String,
    lng: &f64,
    lat: &f64,
    conn: &sqlx::Pool<Postgres>,
) -> Vec<SearchResultDB> {
    let query_string = request
        .split_whitespace()
        .map(|s| {
            let clean: String = s.chars().filter(|c| !"&|!():*".contains(*c)).collect();
            clean
        })
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}:*", s))
        .collect::<Vec<String>>()
        .join(" & ");

    match sqlx::query_as(
            r#"SELECT name, lng, lat FROM (
                    SELECT 
                        street || ', ' || city as name, 
                        CASE
                            WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_X(ST_Transform(geom, 4326))
                            ELSE ST_X(ST_Transform(ST_PointN(geom, 1), 4326))
                        END as lng,
                        CASE
                            WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_Y(ST_Transform(geom, 4326))
                            ELSE ST_Y(ST_Transform(ST_PointN(geom, 1), 4326))
                        END as lat,
                        geom,
                        ROW_NUMBER() OVER(PARTITION BY city, street ORDER BY ar.geom<-> ST_Transform(ST_SetSRID(ST_MakePoint($2, $3), 4326), 3857)) AS rn
                    FROM address_range ar 
                    WHERE tsvector  @@ to_tsquery('french', unaccent($1))
                union
                    select 
                        name || ' ' || coalesce(tags::JSONB->>'addr:street', '') || ' ' || coalesce(tags::JSONB->>'addr:city', '') as name,
                        ST_X(ST_Transform(geom, 4326)) as lng,
                        ST_Y(ST_Transform(geom, 4326)) as lat,
                        geom,
                        1 as rn 
                    from name_query
                    where tsvector  @@ to_tsquery('french', unaccent($1))
            ) t WHERE rn = 1 and name is not null
            order by geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($2, $3), 4326), 3857)
            limit 20;
           "#,
        )
        .bind(query_string)
        .bind(lng)
        .bind(lat)
        .fetch_all(conn)
        .await
        {
            Ok(ar) => ar,
            Err(e) => {
                eprintln!("Error getting SearchResultDB: {}", e);
                Vec::new()
            }
        }
}

pub async fn get_any(lng: &f64, lat: &f64, conn: &sqlx::Pool<Postgres>) -> Vec<SearchResultDB> {
    match sqlx::query_as(
            r#"SELECT name, lng, lat FROM (
                    SELECT 
                        street || ', ' || city as name, 
                        CASE
                            WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_X(ST_Transform(geom, 4326))
                            ELSE ST_X(ST_Transform(ST_PointN(geom, 1), 4326))
                        END as lng,
                        CASE
                            WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_Y(ST_Transform(geom, 4326))
                            ELSE ST_Y(ST_Transform(ST_PointN(geom, 1), 4326))
                        END as lat,
                        geom,
                        ROW_NUMBER() OVER(PARTITION BY city, street ORDER BY ar.geom<-> ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857)) AS rn
                    FROM address_range ar 
                    where ST_DWithin(geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000)
                    union
                    select 
                        distinct on (name || ' ' || coalesce(tags::JSONB->>'addr:street', '') || ' ' || coalesce(tags::JSONB->>'addr:city', ''))
                        name || ' ' || coalesce(tags::JSONB->>'addr:street', '') || ' ' || coalesce(tags::JSONB->>'addr:city', '') as name,
                        ST_X(ST_Transform(geom, 4326)) as lng,
                        ST_Y(ST_Transform(geom, 4326)) as lat,
                        geom,
                        1 as rn
                    from name_query
                    where ST_DWithin(geom, ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857), 1000)
            ) t WHERE name is not null
            order by geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($1, $2), 4326), 3857)
            limit 20;
           "#,
        )
        .bind(lng)
        .bind(lat)
        .fetch_all(conn)
        .await
        {
            Ok(ar) => ar,
            Err(e) => {
                eprintln!("Error getting SearchResultDB: {}", e);
                Vec::new()
            }
        }
}

pub async fn get_with_adress(
    number: &i64,
    request: &String,
    lng: &f64,
    lat: &f64,
    conn: &sqlx::Pool<Postgres>,
) -> Vec<SearchResultDB> {
    let query_string = request
        .split_whitespace()
        .map(|s| {
            let clean: String = s.chars().filter(|c| !"&|!():*".contains(*c)).collect();
            clean
        })
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}:*", s))
        .collect::<Vec<String>>()
        .join(" & ");
    let odd_even = if number % 2 == 0 { "even" } else { "odd" };
    let r = match sqlx::query_as(
        r#"select * 
            from (
                select distinct on ($2 || ' ' || street || ', ' || COALESCE(city,''))
                    $2 || ' ' || street || ', ' || COALESCE(city,'') as name,
                    CASE
                        WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_X(ST_Transform(st_centroid(geom), 4326))
                        ELSE ST_X(ST_Transform(st_centroid(geom), 4326))
                    END as lng,
                    CASE
                        WHEN ST_GeometryType(geom) = 'ST_Point' THEN ST_Y(ST_Transform(st_centroid(geom), 4326))
                        ELSE ST_Y(ST_Transform(st_centroid(geom), 4326))
                    END as lat,
                    st_centroid(geom) as geom
                from address_range ar 
                where tsvector  @@ to_tsquery('french', unaccent($1)) and
                    (start <= $2 and "end" >= $2 or start >= $2 and "end" <= $2 )and
                    odd_even = $3
                order by $2 || ' ' || street || ', ' || COALESCE(city,'')
            ) t
            order by
            geom<-> ST_Transform(ST_SetSRID(ST_MakePoint($4, $5), 4326), 3857)"#,
    )
    .bind(query_string)
    .bind(number)
    .bind(odd_even)
    .bind(lng)
    .bind(lat)
    .fetch_all(conn)
    .await
    {
        Ok(ar) => ar,
        Err(e) => {
            eprintln!("Error getting SearchResultDB 2: {}", e);
            Vec::new()
        }
    };
    println!("get_with_adress: {:?}", r);
    r
}
