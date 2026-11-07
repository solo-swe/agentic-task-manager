//! Names for sprints created without one (the Python atm's `common/sprint_names.py`; docs/sprints-and-releases-design.md
//! SR-2.9), by `meta.sprint_names`: a historical person's name not yet used in the project (`people`), or the time it
//! was created (`time`).
//!
//! The names are plain ASCII letters, spaces, hyphens and dots, without apostrophes: agents pass sprint names in shell
//! commands, inside single quotes.

use std::hash::{Hash, Hasher};

/// The people sprints are named after: the Python atm's list, in its order.
#[rustfmt::skip]
pub const PEOPLE: &[&str] = &[
    // Scientists and mathematicians
    "Ada Lovelace", "Alan Turing", "Albert Einstein", "Alfred Wegener", "Alexander Fleming", "Alexander von Humboldt",
    "Alhazen", "Andre-Marie Ampere", "Antoine Lavoisier", "Archimedes", "Barbara McClintock",
    "Benjamin Banneker", "Blaise Pascal", "Carl Friedrich Gauss", "Carl Linnaeus", "Caroline Herschel",
    "Charles Babbage", "Charles Darwin", "Chien-Shiung Wu", "Claude Shannon", "Dmitri Mendeleev", "Dorothy Hodgkin",
    "Edmond Halley", "Edwin Hubble", "Emmy Noether", "Enrico Fermi", "Ernest Rutherford", "Euclid", "Evariste Galois",
    "Florence Nightingale", "Galileo Galilei", "George Washington Carver", "Gertrude Elion", "Grace Hopper",
    "Gregor Mendel", "Hedy Lamarr", "Henrietta Leavitt", "Hypatia", "Ibn Sina", "Isaac Newton", "James Clerk Maxwell",
    "Jagadish Chandra Bose", "Johannes Kepler", "John von Neumann", "Jonas Salk", "Katherine Johnson",
    "Leonhard Euler", "Lise Meitner", "Louis Pasteur", "Mae Jemison", "Marie Curie", "Mary Anning",
    "Mary Somerville", "Max Planck", "Michael Faraday", "Muhammad al-Khwarizmi", "Niels Bohr", "Nicolaus Copernicus",
    "Nikola Tesla", "Omar Khayyam", "Pierre Curie", "Rachel Carson", "Richard Feynman", "Robert Hooke",
    "Rosalind Franklin", "Santiago Ramon y Cajal", "Sophie Germain", "Srinivasa Ramanujan", "Stephen Hawking",
    "Subrahmanyan Chandrasekhar", "Tu Youyou", "Tycho Brahe", "Vera Rubin", "Werner Heisenberg", "Zhang Heng",
    "Alessandro Volta", "Anders Celsius", "Christiaan Huygens", "Daniel Bernoulli", "Erwin Schrodinger",
    "Fibonacci", "Gottfried Leibniz", "Henri Poincare", "Hermann von Helmholtz", "Jean-Baptiste Lamarck",
    "Joseph Fourier", "Kurt Godel", "Lord Kelvin", "Maria Goeppert Mayer", "Paul Dirac", "Pierre-Simon Laplace",
    "Ronald Ross", "Seki Takakazu", "Wilhelm Rontgen", "William Harvey", "Annie Jump Cannon", "Cecilia Payne",
    "Edward Jenner", "Hans Christian Orsted", "Joseph Lister", "Lev Landau", "Linus Pauling",
    "Ludwig Boltzmann", "Robert Boyle", "Satyendra Nath Bose", "Wangari Maathai",
    // Engineers and inventors
    "Alexander Graham Bell", "Charles Kettering", "Edith Clarke", "Frank Whittle", "Granville Woods", "Gustave Eiffel",
    "Isambard Brunel", "James Watt", "Johannes Gutenberg", "Karl Benz", "Lillian Gilbreth", "Margaret Hamilton",
    "Nikolaus Otto", "Orville Wright", "Rudolf Diesel", "Samuel Morse", "Thomas Edison", "Wilbur Wright",
    "Yuri Gagarin", "Valentina Tereshkova", "Sally Ride", "Robert Goddard", "Konstantin Tsiolkovsky",
    // Explorers and travellers
    "Amelia Earhart", "Abel Tasman", "Bartolomeu Dias", "Ernest Shackleton", "Ferdinand Magellan", "Fridtjof Nansen",
    "Gertrude Bell", "Ibn Battuta", "Isabella Bird", "James Cook", "Leif Erikson", "Matthew Henson", "Marco Polo",
    "Mary Kingsley", "Roald Amundsen", "Sacagawea", "Tenzing Norgay", "Edmund Hillary", "Zheng He", "Xuanzang",
    "Vasco da Gama", "Robert Falcon Scott", "Nellie Bly", "Freya Stark", "Jacques Cousteau", "Thor Heyerdahl",
    // Writers and poets
    "Agatha Christie", "Anton Chekhov", "Charles Dickens", "Charlotte Bronte", "Emily Bronte", "Emily Dickinson",
    "Franz Kafka", "Fyodor Dostoevsky", "Gabriel Garcia Marquez", "George Eliot", "Hans Christian Andersen",
    "Homer", "Italo Calvino", "Jane Austen", "Jorge Luis Borges", "Jules Verne", "Leo Tolstoy", "Mark Twain",
    "Mary Shelley", "Miguel de Cervantes", "Murasaki Shikibu", "Naguib Mahfouz", "Nazim Hikmet", "Oscar Wilde",
    "Pablo Neruda", "Rabindranath Tagore", "Rumi", "Sappho", "Toni Morrison", "Victor Hugo", "Virginia Woolf",
    "Walt Whitman", "William Shakespeare", "Yunus Emre", "Chinua Achebe", "Dante Alighieri", "Edgar Allan Poe",
    "Geoffrey Chaucer", "Hafez", "Herman Melville", "James Joyce", "Johann Wolfgang von Goethe", "Langston Hughes",
    "Li Bai", "Du Fu", "Matsuo Basho", "Maya Angelou", "Rainer Maria Rilke", "Ursula K. Le Guin", "Wislawa Szymborska",
    "Alexandre Dumas", "Arthur Conan Doyle", "Astrid Lindgren", "Bertolt Brecht", "Gustave Flaubert", "H. G. Wells",
    "Honore de Balzac", "Ivan Turgenev", "Lewis Carroll", "Machado de Assis", "Marcel Proust", "Nikolai Gogol",
    "Orhan Veli", "Robert Louis Stevenson", "Sei Shonagon", "Selma Lagerlof", "Sylvia Plath", "Zora Neale Hurston",
    // Philosophers and thinkers
    "Aristotle", "Baruch Spinoza", "Confucius", "Hannah Arendt", "Ibn Khaldun", "Immanuel Kant",
    "John Locke", "Laozi", "Mary Wollstonecraft", "Michel de Montaigne", "Plato", "Rene Descartes", "Simone de Beauvoir",
    "Socrates", "Thales", "Voltaire", "Zeno", "Adam Smith", "David Hume", "Epicurus", "Mencius", "Pythagoras",
    // Artists and architects
    "Artemisia Gentileschi", "Claude Monet", "Diego Velazquez", "Frida Kahlo", "Georgia OKeeffe", "Hokusai",
    "Johannes Vermeer", "Leonardo da Vinci", "Michelangelo", "Mimar Sinan", "Pablo Picasso", "Paul Cezanne",
    "Rembrandt", "Sandro Botticelli", "Vincent van Gogh", "Wassily Kandinsky", "Zaha Hadid", "Antoni Gaudi",
    "Filippo Brunelleschi", "Frank Lloyd Wright", "Gustav Klimt", "Henri Matisse", "Hilma af Klint", "Paul Klee",
    "Raphael", "Rosa Bonheur", "Titian", "Utagawa Hiroshige", "Berthe Morisot", "Edgar Degas", "Le Corbusier",
    // Composers and musicians
    "Antonio Vivaldi", "Clara Schumann", "Claude Debussy", "Duke Ellington", "Fanny Mendelssohn", "Frederic Chopin",
    "George Gershwin", "Hildegard of Bingen", "Igor Stravinsky", "Johann Sebastian Bach", "Ludwig van Beethoven",
    "Maurice Ravel", "Nina Simone", "Pyotr Tchaikovsky", "Scott Joplin", "Wolfgang Amadeus Mozart", "Ella Fitzgerald",
    "Louis Armstrong", "Bela Bartok", "Edvard Grieg", "Franz Schubert", "Giuseppe Verdi", "Jean Sibelius",
    "Joseph Haydn", "Sergei Rachmaninoff", "Antonin Dvorak", "Billie Holiday", "Miles Davis", "Umm Kulthum",
];

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `May 12-10:38pm`: when the sprint was created, from the local time's month (1–12), day, hour and minute.
#[must_use]
pub fn time_name(month: i64, day: i64, hour: i64, minute: i64) -> String {
    let month = usize::try_from(month - 1)
        .ok()
        .and_then(|m| MONTHS.get(m))
        .unwrap_or(&"Jan");
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    let half = if hour < 12 { "am" } else { "pm" };
    format!("{month} {day}-{twelve}:{minute:02}{half}")
}

/// A number from a seed text (`ATM_SEED`, for tests) or, without one, from the clock and the process.
fn seed_number(seed: Option<&str>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Some(seed) = seed {
        seed.hash(&mut hasher);
    } else {
        std::time::SystemTime::now().hash(&mut hasher);
        std::process::id().hash(&mut hasher);
    }
    hasher.finish()
}

/// A new sprint name by `naming` (`meta.sprint_names`), not one of `taken` (compared case-insensitively). `local` is
/// the local time's (month, day, hour, minute), for `time`; `seed` fixes the pick among the people (`ATM_SEED`).
#[must_use]
pub fn generate_sprint_name(
    naming: &str,
    taken: &[String],
    local: (i64, i64, i64, i64),
    seed: Option<&str>,
) -> String {
    let used: Vec<String> = taken.iter().map(|n| n.to_lowercase()).collect();
    let is_used = |name: &str| used.contains(&name.to_lowercase());
    if naming == "time" {
        let base = time_name(local.0, local.1, local.2, local.3);
        let (mut name, mut n) = (base.clone(), 1);
        while is_used(&name) {
            n += 1;
            name = format!("{base}-{n}");
        }
        return name;
    }
    let mut free: Vec<String> = PEOPLE
        .iter()
        .filter(|p| !is_used(p))
        .map(|p| (*p).to_string())
        .collect();
    let mut round = 1;
    while free.is_empty() {
        // Every name taken (a project with hundreds of sprints): the same names again, numbered
        round += 1;
        free = PEOPLE
            .iter()
            .map(|p| format!("{p} {round}"))
            .filter(|p| !is_used(p))
            .collect();
    }
    let at = usize::try_from(seed_number(seed) % free.len() as u64).unwrap_or(0);
    free.swap_remove(at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_name_reads_as_a_clock_does() {
        assert_eq!(time_name(5, 12, 22, 38), "May 12-10:38pm");
        assert_eq!(time_name(1, 3, 0, 5), "Jan 3-12:05am");
        assert_eq!(time_name(7, 1, 12, 0), "Jul 1-12:00pm");
    }

    #[test]
    fn a_name_is_never_one_taken() {
        let taken = vec!["may 12-10:38pm".to_string()];
        assert_eq!(
            generate_sprint_name("time", &taken, (5, 12, 22, 38), None),
            "May 12-10:38pm-2"
        );
        let all: Vec<String> = PEOPLE.iter().map(|p| (*p).to_string()).collect();
        let name = generate_sprint_name("people", &all, (1, 1, 0, 0), Some("7"));
        assert!(name.ends_with(" 2"), "{name}");
        let one = generate_sprint_name("people", &[], (1, 1, 0, 0), Some("7"));
        assert_eq!(
            one,
            generate_sprint_name("people", &[], (1, 1, 0, 0), Some("7"))
        );
        assert!(
            one.bytes()
                .all(|b| b.is_ascii_alphabetic() || b" .-".contains(&b)),
            "{one}"
        );
    }
}
