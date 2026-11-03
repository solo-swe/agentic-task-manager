"""Names for sprints created without one (docs/sprints-and-releases-design.md SR-2.9), by meta.sprint_names: a
historical person's name not yet used in the project (`people`), or the time it was created (`time`).

The names are plain ASCII letters, spaces, hyphens and dots, without apostrophes: agents pass sprint names in shell
commands, inside single quotes."""

import os
import random

from .core import now

PEOPLE = (
    # Scientists and mathematicians
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
    # Engineers and inventors
    "Alexander Graham Bell", "Charles Kettering", "Edith Clarke", "Frank Whittle", "Granville Woods", "Gustave Eiffel",
    "Isambard Brunel", "James Watt", "Johannes Gutenberg", "Karl Benz", "Lillian Gilbreth", "Margaret Hamilton",
    "Nikolaus Otto", "Orville Wright", "Rudolf Diesel", "Samuel Morse", "Thomas Edison", "Wilbur Wright",
    "Yuri Gagarin", "Valentina Tereshkova", "Sally Ride", "Robert Goddard", "Konstantin Tsiolkovsky",
    # Explorers and travellers
    "Amelia Earhart", "Abel Tasman", "Bartolomeu Dias", "Ernest Shackleton", "Ferdinand Magellan", "Fridtjof Nansen",
    "Gertrude Bell", "Ibn Battuta", "Isabella Bird", "James Cook", "Leif Erikson", "Matthew Henson", "Marco Polo",
    "Mary Kingsley", "Roald Amundsen", "Sacagawea", "Tenzing Norgay", "Edmund Hillary", "Zheng He", "Xuanzang",
    "Vasco da Gama", "Robert Falcon Scott", "Nellie Bly", "Freya Stark", "Jacques Cousteau", "Thor Heyerdahl",
    # Writers and poets
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
    # Philosophers and thinkers
    "Aristotle", "Baruch Spinoza", "Confucius", "Hannah Arendt", "Ibn Khaldun", "Immanuel Kant",
    "John Locke", "Laozi", "Mary Wollstonecraft", "Michel de Montaigne", "Plato", "Rene Descartes", "Simone de Beauvoir",
    "Socrates", "Thales", "Voltaire", "Zeno", "Adam Smith", "David Hume", "Epicurus", "Mencius", "Pythagoras",
    # Artists and architects
    "Artemisia Gentileschi", "Claude Monet", "Diego Velazquez", "Frida Kahlo", "Georgia OKeeffe", "Hokusai",
    "Johannes Vermeer", "Leonardo da Vinci", "Michelangelo", "Mimar Sinan", "Pablo Picasso", "Paul Cezanne",
    "Rembrandt", "Sandro Botticelli", "Vincent van Gogh", "Wassily Kandinsky", "Zaha Hadid", "Antoni Gaudi",
    "Filippo Brunelleschi", "Frank Lloyd Wright", "Gustav Klimt", "Henri Matisse", "Hilma af Klint", "Paul Klee",
    "Raphael", "Rosa Bonheur", "Titian", "Utagawa Hiroshige", "Berthe Morisot", "Edgar Degas", "Le Corbusier",
    # Composers and musicians
    "Antonio Vivaldi", "Clara Schumann", "Claude Debussy", "Duke Ellington", "Fanny Mendelssohn", "Frederic Chopin",
    "George Gershwin", "Hildegard of Bingen", "Igor Stravinsky", "Johann Sebastian Bach", "Ludwig van Beethoven",
    "Maurice Ravel", "Nina Simone", "Pyotr Tchaikovsky", "Scott Joplin", "Wolfgang Amadeus Mozart", "Ella Fitzgerald",
    "Louis Armstrong", "Bela Bartok", "Edvard Grieg", "Franz Schubert", "Giuseppe Verdi", "Jean Sibelius",
    "Joseph Haydn", "Sergei Rachmaninoff", "Antonin Dvorak", "Billie Holiday", "Miles Davis", "Umm Kulthum",
)

MONTHS = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")


def time_name(when):
    """`May 12-10:38pm`: when the sprint was created, in local time."""
    hour = when.hour % 12 or 12
    return f"{MONTHS[when.month - 1]} {when.day}-{hour}:{when.minute:02d}{'am' if when.hour < 12 else 'pm'}"


def generate_sprint_name(naming, taken):
    """A new sprint name by `naming` (meta.sprint_names), not one of `taken` (compared case-insensitively). The time
    is atm's clock in the local time zone (ATM_NOW and TZ in tests); ATM_SEED fixes the random pick, for tests."""
    used = {name.casefold() for name in taken}
    if naming == "time":
        base = time_name(now().astimezone())
        name, n = base, 1
        while name.casefold() in used:
            n += 1
            name = f"{base}-{n}"
        return name
    free, round_ = [p for p in PEOPLE if p.casefold() not in used], 1
    while not free:
        # Every name taken (a project with hundreds of sprints): the same names again, numbered
        round_ += 1
        free = [f"{p} {round_}" for p in PEOPLE if f"{p} {round_}".casefold() not in used]
    seed = os.environ.get("ATM_SEED")
    return (random.Random(seed) if seed else random.Random()).choice(free)
