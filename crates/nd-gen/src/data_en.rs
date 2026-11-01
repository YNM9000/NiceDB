pub const MALE: &[&str] = &[
    "Adam", "Alexander", "Andrew", "Anthony", "Benjamin", "Charles", "Christopher", "Daniel", "David",
    "Edward", "Ethan", "George", "Harry", "Henry", "Jack", "Jacob", "James", "Jason", "John",
    "Jonathan", "Joseph", "Joshua", "Kevin", "Leo", "Liam", "Lucas", "Mark", "Matthew", "Michael",
    "Nathan", "Nicholas", "Noah", "Oliver", "Oscar", "Patrick", "Paul", "Peter", "Richard", "Robert",
    "Ryan", "Samuel", "Scott", "Simon", "Stephen", "Thomas", "Timothy", "William",
];

pub const FEMALE: &[&str] = &[
    "Abigail", "Alice", "Amelia", "Amy", "Anna", "Ava", "Charlotte", "Chloe", "Claire", "Daisy",
    "Elizabeth", "Ella", "Emily", "Emma", "Evelyn", "Florence", "Grace", "Hannah", "Harper", "Isabella",
    "Isla", "Jessica", "Julia", "Katherine", "Laura", "Lily", "Lucy", "Madison", "Megan", "Mia",
    "Natalie", "Olivia", "Poppy", "Rachel", "Rebecca", "Rose", "Ruby", "Samantha", "Sarah", "Sophie",
    "Victoria", "Zoe",
];

pub const SURNAMES_UK: &[&str] = &[
    "Smith", "Jones", "Taylor", "Brown", "Williams", "Wilson", "Johnson", "Davies", "Robinson", "Wright",
    "Thompson", "Evans", "Walker", "White", "Roberts", "Green", "Hall", "Wood", "Jackson", "Clarke",
    "Patel", "Khan", "Lewis", "James", "Phillips", "Mason", "Mitchell", "Rose", "Davis", "Rodgers",
    "Cooper", "Hughes", "Turner", "Harris", "Ward", "Baker", "Bennett", "Carter", "Price", "Murphy",
    "Morgan", "Kelly", "Shaw", "Hill", "Moore", "Fletcher", "Marshall", "Owen", "Palmer", "Knight",
];

pub const SURNAMES_US: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis", "Rodriguez",
    "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson", "Thomas", "Taylor", "Moore",
    "Jackson", "Martin", "Lee", "Perez", "Thompson", "White", "Harris", "Sanchez", "Clark", "Ramirez",
    "Lewis", "Robinson", "Walker", "Young", "Allen", "King", "Wright", "Scott", "Torres", "Nguyen",
    "Hill", "Flores", "Green", "Adams", "Nelson", "Baker", "Hall", "Rivera", "Campbell", "Mitchell",
    "Carter", "Roberts", "Kim", "Chen",
];

pub const CITIES_UK: &[(&str, &str, &str, u32)] = &[
    ("London", "Greater London", "SW", 20),
    ("London", "Greater London", "N", 22),
    ("London", "Greater London", "E", 18),
    ("Manchester", "Greater Manchester", "M", 40),
    ("Birmingham", "West Midlands", "B", 90),
    ("Leeds", "West Yorkshire", "LS", 29),
    ("Sheffield", "South Yorkshire", "S", 36),
    ("Bristol", "Bristol", "BS", 41),
    ("Liverpool", "Merseyside", "L", 38),
    ("Newcastle upon Tyne", "Tyne and Wear", "NE", 15),
    ("Nottingham", "Nottinghamshire", "NG", 22),
    ("Leicester", "Leicestershire", "LE", 19),
    ("Brighton", "East Sussex", "BN", 3),
    ("Oxford", "Oxfordshire", "OX", 4),
    ("Cambridge", "Cambridgeshire", "CB", 5),
    ("York", "North Yorkshire", "YO", 32),
    ("Bath", "Somerset", "BA", 2),
    ("Norwich", "Norfolk", "NR", 14),
    ("Exeter", "Devon", "EX", 4),
    ("Reading", "Berkshire", "RG", 31),
    ("Cardiff", "South Glamorgan", "CF", 24),
    ("Edinburgh", "City of Edinburgh", "EH", 17),
    ("Glasgow", "Glasgow City", "G", 46),
    ("Aberdeen", "Aberdeen City", "AB", 25),
    ("Belfast", "County Antrim", "BT", 17),
];

pub const CITIES_US: &[(&str, &str, &str, &str)] = &[
    ("New York", "NY", "100", "212"),
    ("Brooklyn", "NY", "112", "718"),
    ("Los Angeles", "CA", "900", "213"),
    ("San Francisco", "CA", "941", "415"),
    ("San Diego", "CA", "921", "619"),
    ("Chicago", "IL", "606", "312"),
    ("Houston", "TX", "770", "713"),
    ("Austin", "TX", "787", "512"),
    ("Dallas", "TX", "752", "214"),
    ("Phoenix", "AZ", "850", "602"),
    ("Philadelphia", "PA", "191", "215"),
    ("Pittsburgh", "PA", "152", "412"),
    ("Seattle", "WA", "981", "206"),
    ("Portland", "OR", "972", "503"),
    ("Denver", "CO", "802", "303"),
    ("Boston", "MA", "021", "617"),
    ("Atlanta", "GA", "303", "404"),
    ("Miami", "FL", "331", "305"),
    ("Orlando", "FL", "328", "407"),
    ("Minneapolis", "MN", "554", "612"),
    ("Detroit", "MI", "482", "313"),
    ("Nashville", "TN", "372", "615"),
    ("Columbus", "OH", "432", "614"),
    ("Baltimore", "MD", "212", "410"),
    ("Salt Lake City", "UT", "841", "801"),
    ("Raleigh", "NC", "276", "919"),
];

pub const STREETS_UK: &[&str] = &[
    "Station", "High", "Church", "Victoria", "Mill", "Park", "London", "Queen's", "King's", "Manor",
    "Albert", "Chapel", "Green", "Grange", "Kingsway", "Windsor", "York", "Springfield", "Beech",
    "Orchard", "Meadow", "Riverside", "Elm", "Oak", "Willow", "Cedar", "Holly", "Ashford",
];

pub const STREET_KINDS_UK: &[&str] = &[
    "Road", "Road", "Street", "Street", "Lane", "Avenue", "Close", "Drive", "Way", "Gardens", "Crescent",
    "Place", "Terrace", "Grove",
];

pub const STREETS_US: &[&str] = &[
    "Maple", "Oak", "Pine", "Cedar", "Elm", "Washington", "Lake", "Hill", "Main", "Park", "Sunset",
    "Jefferson", "Lincoln", "Madison", "Franklin", "Highland", "River", "Spring", "Ridge", "Walnut",
    "Chestnut", "Willow", "Meadow", "Forest", "Broad", "Market", "Church", "Center",
];

pub const STREET_KINDS_US: &[&str] = &[
    "Street", "Street", "Avenue", "Avenue", "Road", "Drive", "Lane", "Boulevard", "Court", "Place",
    "Way", "Parkway",
];

pub const COMPANY_WORDS: &[&str] = &[
    "Northwind", "Bluebell", "Harbour", "Summit", "Brightwater", "Ironbridge", "Kestrel", "Oakfield",
    "Redwood", "Silverline", "Westbrook", "Highgate", "Pinecrest", "Riverstone", "Fairview",
    "Greenleaf", "Stonegate", "Larkspur", "Copperfield", "Evergreen",
];

pub const COMPANY_TRADES: &[&str] = &[
    "Logistics", "Engineering", "Software", "Consulting", "Foods", "Construction", "Trading",
    "Design", "Analytics", "Supplies", "Systems", "Joinery", "Plumbing", "Electrical", "Media",
    "Healthcare", "Interiors", "Packaging", "Textiles", "Robotics",
];

pub const COMPANY_FORMS_UK: &[&str] = &["Ltd", "Ltd", "Ltd", "PLC", "LLP", "Group Ltd"];

pub const COMPANY_FORMS_US: &[&str] = &["Inc.", "Inc.", "LLC", "LLC", "Corp.", "Co."];

pub const LOREM: &[&str] = &[
    "order", "delivery", "customer", "warehouse", "invoice", "discount", "pallet", "urgent", "check",
    "note", "supplier", "payment", "shipment", "stock", "account", "request", "review", "approved",
    "pending", "return", "item", "batch", "quote", "contract", "schedule", "service", "visit",
    "report", "update", "monthly", "agreed", "confirmed", "please", "call", "back", "before", "after",
    "the", "for", "with", "and", "next", "week", "office", "site", "team", "price", "label",
];

pub const INWARD: &[u8] = b"ABDEFGHJLNPQRSTUWXYZ";
