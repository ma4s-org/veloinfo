#!/usr/bin/bash
set -e
set -o pipefail

# ==============================================================================
# CONFIGURATION
# ==============================================================================
PSQL_CMD="psql -h db -U postgres -d carte -v ON_ERROR_STOP=1"

SRTM_FILE="/tmp/srtm_22_03/srtm_22_03.tif"
SRTM_URL="https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_22_03.zip"

# Check if SRTM file exists, if not download it
if [ ! -f "$SRTM_FILE" ]; then
    echo "SRTM file not found, downloading..."
    mkdir -p /tmp/srtm_22_03
    
    # Download the zip file
    echo "Downloading SRTM data from $SRTM_URL..."
    wget -O /tmp/srtm_22_03.zip "$SRTM_URL"
    
    # Extract the zip file
    echo "Extracting SRTM data..."
    unzip -o /tmp/srtm_22_03.zip -d /tmp/srtm_22_03/
    
    # Clean up zip file
    rm /tmp/srtm_22_03.zip
    
    echo "✓ SRTM data downloaded and extracted"
else
    echo "✓ SRTM file already exists"
fi

echo "SRTM Elevation Data Import Script"
echo "===================================="
echo ""

echo "✓ raster2pgsql is installed"
echo ""

# Create public schema if it doesn't exist (no import schema needed)
echo "Ensuring public schema is ready..."
$PSQL_CMD -c "CREATE SCHEMA IF NOT EXISTS public;"
echo "✓ Public schema ready"
echo ""

# Enable PostGIS Raster extension
echo "Enabling PostGIS Raster extension..."
$PSQL_CMD -c "CREATE EXTENSION IF NOT EXISTS postgis_raster;" 2>/dev/null || echo "Note: PostGIS Raster extension may already be enabled or requires superuser privileges"
echo "✓ PostGIS Raster extension ready"
echo ""

# Generate SQL script
echo "Generating SQL script from GeoTIFF (tiled for performance)..."
SQL_FILE="/tmp/srtm_load.sql"
# -t: tile size, -d: drop table, -I: create spatial index, -C: add constraints, -M: analyze
raster2pgsql -t 256x256 -d -I -C -M "$SRTM_FILE" public.srtm_elevation > "$SQL_FILE"
echo "✓ SQL script generated at $SQL_FILE"
echo ""

# Load into database
echo "Loading SRTM data into PostgreSQL..."
if ! $PSQL_CMD -f "$SQL_FILE" 2>&1 | grep -v "perl: warning"; then
    echo "Error loading SRTM data!"
    exit 1
fi

# Check if load was successful
TABLE_EXISTS=$($PSQL_CMD -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'srtm_elevation');" | xargs)

if [ "$TABLE_EXISTS" != "t" ]; then
    echo "✗ Error: SRTM table was not created successfully"
    echo "Please ensure PostGIS Raster extension is installed and enabled"
    exit 1
fi

echo "✓ SRTM data loaded successfully"
echo "Optimizing raster indexes..."
$PSQL_CMD -c "CREATE INDEX IF NOT EXISTS srtm_elevation_rast_gist ON public.srtm_elevation USING GIST (ST_ConvexHull(rast));"
$PSQL_CMD -c "ANALYZE public.srtm_elevation;"
echo "✓ Raster index and stats updated"

echo "Building polygon elevation table (this can be VERY large and may take a while)..."
$PSQL_CMD << 'EOF'
DROP TABLE IF EXISTS public.srtm_elevation_polygons;
CREATE TABLE public.srtm_elevation_polygons (
    -- On définit la géométrie directement en 3857
    geom geometry(Polygon, 3857),
    elevation double precision
);

INSERT INTO public.srtm_elevation_polygons (geom, elevation)
SELECT 
    ST_Transform((p).geom, 3857), -- On transforme ici une fois pour toutes
    (p).val
FROM (
    SELECT ST_PixelAsPolygons(rast, 1) AS p
    FROM public.srtm_elevation
) AS polys;

-- L'index GIST sera créé sur du 3857
CREATE INDEX srtm_elevation_polygons_geom_idx ON public.srtm_elevation_polygons USING GIST (geom);
ANALYZE public.srtm_elevation_polygons;
EOF
echo "✓ Polygon elevation table built"
echo ""

# Verify the data
echo "Verifying SRTM data..."
RASTER_COUNT=$($PSQL_CMD -t -c "SELECT COUNT(*) FROM public.srtm_elevation;")

echo "✓ Rasters in database: $RASTER_COUNT"

