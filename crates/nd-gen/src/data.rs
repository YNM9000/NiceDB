pub const MALE: &[&str] = &[
    "Alessandro", "Andrea", "Antonio", "Carlo", "Claudio", "Corrado", "Daniele", "Dario", "Davide",
    "Domenico", "Edoardo", "Emanuele", "Enrico", "Fabio", "Federico", "Filippo", "Francesco",
    "Gabriele", "Giacomo", "Gianluca", "Giorgio", "Giovanni", "Giulio", "Giuseppe", "Guido", "Ivan",
    "Jacopo", "Leonardo", "Lorenzo", "Luca", "Luigi", "Marco", "Mario", "Massimo", "Matteo",
    "Mattia", "Michele", "Nicola", "Paolo", "Pietro", "Raffaele", "Riccardo", "Roberto",
    "Salvatore", "Samuele", "Simone", "Stefano", "Tommaso", "Umberto", "Valerio", "Vincenzo",
    "Vittorio",
];

pub const FEMALE: &[&str] = &[
    "Alessandra", "Alice", "Anna", "Arianna", "Aurora", "Beatrice", "Benedetta", "Camilla", "Carla",
    "Caterina", "Chiara", "Cristina", "Daniela", "Elena", "Eleonora", "Elisa", "Emma", "Federica",
    "Francesca", "Gaia", "Giada", "Ginevra", "Giorgia", "Giulia", "Ilaria", "Irene", "Laura",
    "Letizia", "Lucia", "Ludovica", "Margherita", "Maria", "Marta", "Martina", "Matilde", "Monica",
    "Noemi", "Paola", "Rebecca", "Roberta", "Rosa", "Sara", "Serena", "Silvia", "Sofia", "Stefania",
    "Teresa", "Valentina", "Valeria", "Veronica", "Viola", "Vittoria",
];

pub const SURNAMES: &[&str] = &[
    "Rossi", "Russo", "Ferrari", "Esposito", "Bianchi", "Romano", "Colombo", "Ricci", "Marino",
    "Greco", "Bruno", "Gallo", "Conti", "De Luca", "Mancini", "Costa", "Giordano", "Rizzo",
    "Lombardi", "Moretti", "Barbieri", "Fontana", "Santoro", "Mariani", "Rinaldi", "Caruso",
    "Ferrara", "Galli", "Martini", "Leone", "Longo", "Gentile", "Martinelli", "Vitale", "Lombardo",
    "Serra", "Coppola", "De Santis", "D'Angelo", "Marchetti", "Parisi", "Villa", "Conte", "Ferraro",
    "Ferri", "Fabbri", "Bianco", "Marini", "Grasso", "Valentini", "Messina", "Sala", "De Angelis",
    "Gatti", "Pellegrini", "Palumbo", "Sanna", "Farina", "Rizzi", "Monti", "Cattaneo", "Morelli",
    "Amato", "Silvestri", "Mazza", "Testa", "Grassi", "Pellegrino", "Carbone", "Giuliani",
    "Benedetti", "Barone", "Rossetti", "Caputo", "Montanari", "Guerra", "Palmieri", "Bernardi",
    "Martino", "Fiore", "De Rosa", "Ferretti", "Bellini", "Basile", "Riva", "Donati", "Piras",
    "Vitali", "Battaglia", "Sartori", "Neri", "Costantini", "Milani", "Pagano", "Ruggiero",
    "Sorrentino", "D'Amico", "Orlando", "Negri", "Bassi", "Pace", "Fumagalli", "Tosi", "Bertolini",
    "Zanetti", "Brambilla", "Ghezzi", "Zanella", "Cocco", "Melis", "Pozzi", "Bonetti", "Manca",
    "Valente", "Fadda",
];

pub const CITIES: &[(&str, &str, &str, &str)] = &[
    ("Roma", "RM", "00184", "H501"),
    ("Milano", "MI", "20121", "F205"),
    ("Napoli", "NA", "80133", "F839"),
    ("Torino", "TO", "10121", "L219"),
    ("Palermo", "PA", "90133", "G273"),
    ("Genova", "GE", "16121", "D969"),
    ("Bologna", "BO", "40121", "A944"),
    ("Firenze", "FI", "50122", "D612"),
    ("Bari", "BA", "70121", "A662"),
    ("Catania", "CT", "95121", "C351"),
    ("Venezia", "VE", "30121", "L736"),
    ("Verona", "VR", "37121", "L781"),
    ("Messina", "ME", "98122", "F158"),
    ("Padova", "PD", "35121", "G224"),
    ("Trieste", "TS", "34121", "L424"),
    ("Brescia", "BS", "25121", "B157"),
    ("Parma", "PR", "43121", "G337"),
    ("Taranto", "TA", "74121", "L049"),
    ("Prato", "PO", "59100", "G999"),
    ("Modena", "MO", "41121", "F257"),
    ("Reggio Emilia", "RE", "42121", "H223"),
    ("Perugia", "PG", "06121", "G478"),
    ("Livorno", "LI", "57123", "E625"),
    ("Ravenna", "RA", "48121", "H199"),
    ("Cagliari", "CA", "09124", "B354"),
    ("Foggia", "FG", "71121", "D643"),
    ("Rimini", "RN", "47921", "H294"),
    ("Salerno", "SA", "84121", "H703"),
    ("Ferrara", "FE", "44121", "D548"),
    ("Sassari", "SS", "07100", "I452"),
    ("Latina", "LT", "04100", "E472"),
    ("Monza", "MB", "20900", "F704"),
    ("Siracusa", "SR", "96100", "I754"),
    ("Pescara", "PE", "65121", "G482"),
    ("Bergamo", "BG", "24121", "A794"),
    ("Trento", "TN", "38121", "L378"),
    ("Vicenza", "VI", "36100", "L840"),
    ("Terni", "TR", "05100", "L117"),
    ("Bolzano", "BZ", "39100", "A952"),
    ("Novara", "NO", "28100", "F952"),
    ("Piacenza", "PC", "29121", "G535"),
    ("Ancona", "AN", "60121", "A271"),
    ("Udine", "UD", "33100", "L483"),
    ("Arezzo", "AR", "52100", "A390"),
    ("Lecce", "LE", "73100", "E506"),
    ("La Spezia", "SP", "19121", "E463"),
    ("Pisa", "PI", "56121", "G702"),
    ("Como", "CO", "22100", "C933"),
    ("Lucca", "LU", "55100", "E715"),
    ("Siena", "SI", "53100", "I726"),
    ("Aosta", "AO", "11100", "A326"),
    ("Potenza", "PZ", "85100", "G942"),
    ("Campobasso", "CB", "86100", "B519"),
    ("Catanzaro", "CZ", "88100", "C352"),
    ("Matera", "MT", "75100", "F052"),
    ("Cosenza", "CS", "87100", "D086"),
    ("Pavia", "PV", "27100", "G388"),
    ("Cremona", "CR", "26100", "D150"),
    ("Mantova", "MN", "46100", "E897"),
    ("Varese", "VA", "21100", "L682"),
    ("Treviso", "TV", "31100", "L407"),
    ("Asti", "AT", "14100", "A479"),
    ("Cuneo", "CN", "12100", "D205"),
    ("Alessandria", "AL", "15121", "A182"),
];

pub const STREET_KINDS: &[&str] = &[
    "Via", "Via", "Via", "Via", "Viale", "Corso", "Piazza", "Largo", "Vicolo", "Strada",
];

pub const STREETS: &[&str] = &[
    "Roma", "Garibaldi", "Mazzini", "Cavour", "Dante", "Verdi", "Manzoni", "Marconi",
    "XX Settembre", "Vittorio Veneto", "dei Mille", "della Repubblica", "IV Novembre", "Matteotti",
    "Carducci", "Leopardi", "Petrarca", "Galilei", "Volta", "Colombo", "dei Tigli", "delle Rose",
    "dei Pini", "del Lavoro", "della Libertà", "San Martino", "Santa Chiara", "dei Ciliegi",
    "del Mulino", "della Stazione", "del Porto", "delle Industrie", "Cesare Battisti",
    "Nazario Sauro", "Aldo Moro", "Gramsci", "Pascoli", "Puccini", "Rossini", "Bellini", "Diaz",
    "Trento", "Trieste", "Milano", "Torino", "Venezia", "Firenze", "Napoli", "del Commercio",
    "degli Artigiani", "delle Vigne", "dei Castagni", "del Sole", "del Borgo",
];

pub const COMPANY_WORDS: &[&str] = &[
    "Alfa", "Aurora", "Borea", "Cometa", "Delta", "Eridano", "Fenice", "Gemma", "Iride", "Levante",
    "Libeccio", "Maestrale", "Meridiana", "Nettuno", "Orione", "Pegaso", "Polaris", "Quadrifoglio",
    "Sirio", "Tramontana", "Vega", "Zefiro", "Arcobaleno", "Bussola", "Cardo", "Faro", "Girasole",
    "Mosaico", "Olivo", "Quercia", "Sestante", "Torre", "Ulivo", "Vela",
];

pub const COMPANY_TRADES: &[&str] = &[
    "Costruzioni", "Impianti", "Trasporti", "Forniture", "Servizi", "Informatica", "Alimentari",
    "Meccanica", "Arredamenti", "Ceramiche", "Tessile", "Logistica", "Elettronica", "Consulenze",
    "Edilizia", "Serramenti", "Vini", "Stampa", "Ottica", "Termoidraulica",
];

pub const COMPANY_FORMS: &[&str] = &[
    "S.r.l.", "S.r.l.", "S.r.l.", "S.p.A.", "S.n.c.", "S.a.s.", "S.r.l.s.", "Soc. Coop.",
];

pub const DOMAINS: &[&str] = &[
    "example.com", "example.org", "example.net", "esempio.test", "posta.test",
];

pub const LOREM: &[&str] = &[
    "il", "la", "di", "che", "e", "un", "una", "per", "con", "del", "della", "nel", "sul", "tra",
    "come", "anche", "sempre", "ancora", "molto", "poco", "oggi", "domani", "subito", "lavoro",
    "ordine", "consegna", "cliente", "fornitore", "magazzino", "prodotto", "prezzo", "sconto",
    "fattura", "pagamento", "spedizione", "reso", "garanzia", "richiesta", "risposta", "verifica",
    "controllo", "nota", "accordo", "contratto", "progetto", "riunione", "telefonata", "mattina",
    "pomeriggio", "sera", "settimana", "mese", "anno", "giorno", "ufficio", "sede", "filiale",
    "reparto", "squadra", "materiale", "campione", "listino", "catalogo", "scatola", "pallet",
    "corriere", "furgone", "porto", "stazione", "piazza", "strada", "città", "paese", "nuovo",
    "vecchio", "grande", "piccolo", "pronto", "urgente", "completo", "parziale", "regolare",
    "diverso", "chiaro", "semplice", "breve", "lungo", "veloce", "lento", "buono", "arriva",
    "parte", "chiede", "conferma", "aspetta", "manda", "riceve", "controlla", "prepara", "chiude",
    "apre", "segna", "aggiorna", "ricorda", "spiega", "propone",
];

pub const DIGITS: &[u8] = b"0123456789";
pub const ALNUM: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ0123456789";
