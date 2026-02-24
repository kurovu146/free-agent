# Database & SQL

## PostgreSQL Patterns

### Query Optimization
- EXPLAIN ANALYZE truoc khi optimize
- Index cho columns hay WHERE/JOIN/ORDER BY
- Tranh SELECT *, chi lay columns can
- Keyset pagination cho large datasets
- Batch INSERT thay vi INSERT tung row

### Common SQL
```sql
-- Keyset pagination
SELECT * FROM items WHERE id > $1 ORDER BY id LIMIT $2;

-- Upsert
INSERT INTO table (key, value) VALUES ($1, $2)
ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value;

-- JSONB query
SELECT data->>'name' FROM items WHERE data @> '{"type": "weapon"}';
```

### Migration Best Practices
- Moi migration co UP va DOWN
- Them column: luon DEFAULT hoac NULL
- Index: CREATE CONCURRENTLY tren production
- Test tren staging truoc
