-- Active users model (depends on users)
SELECT
  id,
  name,
  email,
  created_at
FROM {{ ref('users') }}
WHERE email IS NOT NULL
