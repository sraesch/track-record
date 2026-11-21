-- Unit catalogue (FR-13a). Every metric, series x-dimension and, where it
-- applies, numeric parameter has a unit, stored as its symbol (text). Symbols
-- found here can be converted within their quantity. Any other symbol is
-- allowed as a custom unit, but it is not converted.
--
-- value in a unit * to_base = value in the base unit of its quantity.
-- '' is the dimensionless unit. Units that need an offset (e.g. °C) cannot be
-- expressed and stay custom units.

CREATE TABLE unit (
    symbol   text PRIMARY KEY CHECK (symbol = btrim(symbol)),
    quantity text NOT NULL CHECK (quantity <> '' AND quantity = btrim(quantity)),
    to_base  double precision NOT NULL CHECK (to_base > 0 AND to_base < 'Infinity'),
    name     text NOT NULL CHECK (btrim(name) <> '')
);

CREATE INDEX unit_quantity_idx ON unit (quantity);

INSERT INTO unit (quantity, symbol, to_base, name) VALUES
    ('time', 'ns',  1e-9,  'nanosecond'),
    ('time', 'us',  1e-6,  'microsecond'),
    ('time', 'µs',  1e-6,  'microsecond'),
    ('time', 'ms',  1e-3,  'millisecond'),
    ('time', 's',   1,     'second'),
    ('time', 'min', 60,    'minute'),
    ('time', 'h',   3600,  'hour'),
    ('time', 'd',   86400, 'day'),

    ('data size', 'bit',  0.125,         'bit'),
    ('data size', 'kbit', 125,           'kilobit'),
    ('data size', 'Mbit', 125000,        'megabit'),
    ('data size', 'Gbit', 125000000,     'gigabit'),
    ('data size', 'B',    1,             'byte'),
    ('data size', 'kB',   1e3,           'kilobyte'),
    ('data size', 'MB',   1e6,           'megabyte'),
    ('data size', 'GB',   1e9,           'gigabyte'),
    ('data size', 'TB',   1e12,          'terabyte'),
    ('data size', 'KiB',  1024,          'kibibyte'),
    ('data size', 'MiB',  1048576,       'mebibyte'),
    ('data size', 'GiB',  1073741824,    'gibibyte'),
    ('data size', 'TiB',  1099511627776, 'tebibyte'),

    ('data rate', 'bit/s',  0.125,      'bit per second'),
    ('data rate', 'kbit/s', 125,        'kilobit per second'),
    ('data rate', 'Mbit/s', 125000,     'megabit per second'),
    ('data rate', 'Gbit/s', 125000000,  'gigabit per second'),
    ('data rate', 'B/s',    1,          'byte per second'),
    ('data rate', 'kB/s',   1e3,        'kilobyte per second'),
    ('data rate', 'MB/s',   1e6,        'megabyte per second'),
    ('data rate', 'GB/s',   1e9,        'gigabyte per second'),
    ('data rate', 'KiB/s',  1024,       'kibibyte per second'),
    ('data rate', 'MiB/s',  1048576,    'mebibyte per second'),
    ('data rate', 'GiB/s',  1073741824, 'gibibyte per second'),

    ('frequency', 'Hz',  1,   'hertz'),
    ('frequency', 'kHz', 1e3, 'kilohertz'),
    ('frequency', 'MHz', 1e6, 'megahertz'),
    ('frequency', 'GHz', 1e9, 'gigahertz'),

    ('frame rate', 'fps', 1, 'frames per second'),

    ('ratio', '',    1,    'dimensionless'),
    ('ratio', '%',   1e-2, 'percent'),
    ('ratio', '‰',   1e-3, 'per mille'),
    ('ratio', 'ppm', 1e-6, 'parts per million'),

    ('count', 'count',    1,   'count'),
    ('count', 'thousand', 1e3, 'thousand'),
    ('count', 'million',  1e6, 'million'),
    ('count', 'billion',  1e9, 'billion'),

    ('length', 'mm', 1e-3, 'millimetre'),
    ('length', 'cm', 1e-2, 'centimetre'),
    ('length', 'm',  1,    'metre'),
    ('length', 'km', 1e3,  'kilometre'),

    ('pixels', 'px',  1,   'pixel'),
    ('pixels', 'Mpx', 1e6, 'megapixel'),

    ('power', 'mW', 1e-3, 'milliwatt'),
    ('power', 'W',  1,    'watt'),
    ('power', 'kW', 1e3,  'kilowatt'),

    ('energy', 'mJ',  1e-3,    'millijoule'),
    ('energy', 'J',   1,       'joule'),
    ('energy', 'kJ',  1e3,     'kilojoule'),
    ('energy', 'mWh', 3.6,     'milliwatt hour'),
    ('energy', 'Wh',  3600,    'watt hour'),
    ('energy', 'kWh', 3600000, 'kilowatt hour');

-- The catalogue only grows, through migrations.
SELECT make_immutable('unit', false);

-- Factor that converts a value from one unit to another: value * factor.
-- NULL if the units cannot be converted: different quantities, or a custom
-- unit that is not in the catalogue. Equal symbols always convert with 1.
CREATE FUNCTION unit_conversion_factor(p_from text, p_to text) RETURNS double precision
    LANGUAGE sql STABLE STRICT AS $$
    SELECT CASE
        WHEN p_from = p_to THEN 1
        ELSE (SELECT f.to_base / t.to_base
              FROM unit f
              JOIN unit t ON t.quantity = f.quantity
              WHERE f.symbol = p_from AND t.symbol = p_to)
    END;
$$;
