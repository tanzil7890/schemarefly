-- Simple users model
SELECT
  id,
  name,
  email,
  created_at
FROM {{ source('raw', 'users') }}
WHERE deleted_at IS NULL
