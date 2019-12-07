SELECT
    id,
    path,
    registrationTime
FROM
    ValidPaths
WHERE
    ca IS NULL -- The ca row is set on .drv and (most) archive derivation's, which we don't care about
    AND id != ?1
    AND id IN (
        SELECT
            reference
        FROM
            Refs
        WHERE
            referrer = ?1
    )
ORDER BY
    registrationTime DESC