SELECT
    id,
    path,
    registrationTime
FROM
    ValidPaths
WHERE
    ca IS NULL -- The ca row is set on .drv and (most) archive derivation's, which we don't care about
    AND path NOT LIKE '%-completions' -- We don't care about shell completions
    AND path NOT LIKE '%.tar.%' -- Sometimes the ca field is not set on tar.* archives
ORDER BY
    registrationTime DESC