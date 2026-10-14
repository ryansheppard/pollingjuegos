from django.db import migrations

# Standard NFL abbreviations; the source list did not provide abbreviations.
TEAMS = (
    ("Arizona Cardinals", "ARI"),
    ("Atlanta Falcons", "ATL"),
    ("Baltimore Ravens", "BAL"),
    ("Buffalo Bills", "BUF"),
    ("Carolina Panthers", "CAR"),
    ("Chicago Bears", "CHI"),
    ("Cincinnati Bengals", "CIN"),
    ("Cleveland Browns", "CLE"),
    ("Dallas Cowboys", "DAL"),
    ("Denver Broncos", "DEN"),
    ("Detroit Lions", "DET"),
    ("Green Bay Packers", "GB"),
    ("Houston Texans", "HOU"),
    ("Indianapolis Colts", "IND"),
    ("Jacksonville Jaguars", "JAX"),
    ("Kansas City Chiefs", "KC"),
    ("Las Vegas Raiders", "LV"),
    ("Los Angeles Chargers", "LAC"),
    ("Los Angeles Rams", "LAR"),
    ("Miami Dolphins", "MIA"),
    ("Minnesota Vikings", "MIN"),
    ("New England Patriots", "NE"),
    ("New Orleans Saints", "NO"),
    ("New York Giants", "NYG"),
    ("New York Jets", "NYJ"),
    ("Philadelphia Eagles", "PHI"),
    ("Pittsburgh Steelers", "PIT"),
    ("San Francisco 49ers", "SF"),
    ("Seattle Seahawks", "SEA"),
    ("Tampa Bay Buccaneers", "TB"),
    ("Tennessee Titans", "TEN"),
    ("Washington Commanders", "WAS"),
)

# Names are stored in given-name-first display order; college is intentionally omitted.
QUARTERBACKS = (
    ("Drew Allar", "Pittsburgh Steelers"),
    ("Josh Allen", "Buffalo Bills"),
    ("Kyle Allen", "Buffalo Bills"),
    ("Tyson Bagent", "Chicago Bears"),
    ("Carson Beck", "Arizona Cardinals"),
    ("Stetson Bennett IV", "Los Angeles Rams"),
    ("Jacoby Brissett", "Arizona Cardinals"),
    ("Joe Burrow", "Cincinnati Bengals"),
    ("Brady Cook", "Miami Dolphins"),
    ("Kirk Cousins", "Las Vegas Raiders"),
    ("Andy Dalton", "Philadelphia Eagles"),
    ("Jalon Daniels", "Tampa Bay Buccaneers"),
    ("Jayden Daniels", "Washington Commanders"),
    ("Sam Darnold", "Seattle Seahawks"),
    ("Tommy DeVito", "New England Patriots"),
    ("Joshua Dobbs", "Detroit Lions"),
    ("Sam Ehlinger", "Denver Broncos"),
    ("Quinn Ewers", "Jacksonville Jaguars"),
    ("Joe Fagnano", "Baltimore Ravens"),
    ("Justin Fields", "Kansas City Chiefs"),
    ("Joe Flacco", "Cincinnati Bengals"),
    ("Jared Goff", "Detroit Lions"),
    ("Taylen Green", "Cleveland Browns"),
    ("Jake Haener", "New York Giants"),
    ("Justin Herbert", "Los Angeles Chargers"),
    ("Will Howard", "Pittsburgh Steelers"),
    ("Sam Howell", "Dallas Cowboys"),
    ("Tyler Huntley", "Baltimore Ravens"),
    ("Jalen Hurts", "Philadelphia Eagles"),
    ("Lamar Jackson", "Baltimore Ravens"),
    ("Josh Johnson", "Cincinnati Bengals"),
    ("Daniel Jones", "Indianapolis Colts"),
    ("Mac Jones", "San Francisco 49ers"),
    ("Athan Kaliakmanis", "Washington Commanders"),
    ("Case Keenum", "Chicago Bears"),
    ("Haynes King", "Carolina Panthers"),
    ("Cade Klubnik", "New York Jets"),
    ("Trey Lance", "Los Angeles Chargers"),
    ("Trevor Lawrence", "Jacksonville Jaguars"),
    ("Riley Leonard", "Indianapolis Colts"),
    ("Will Levis", "New York Jets"),
    ("Drew Lock", "Seattle Seahawks"),
    ("Jordan Love", "Green Bay Packers"),
    ("Patrick Mahomes", "Kansas City Chiefs"),
    ("Marcus Mariota", "Washington Commanders"),
    ("Drake Maye", "New England Patriots"),
    ("Baker Mayfield", "Tampa Bay Buccaneers"),
    ("J.J. McCarthy", "New York Giants"),
    ("Kyle McCord", "Miami Dolphins"),
    ("Tanner McKee", "Philadelphia Eagles"),
    ("Fernando Mendoza", "Las Vegas Raiders"),
    ("Davis Mills", "Houston Texans"),
    ("Jalen Milroe", "Seattle Seahawks"),
    ("Gardner Minshew II", "Arizona Cardinals"),
    ("Behren Morton", "New England Patriots"),
    ("Miller Moss", "Chicago Bears"),
    ("Nick Mullens", "Jacksonville Jaguars"),
    ("Kyler Murray", "Minnesota Vikings"),
    ("Bo Nix", "Denver Broncos"),
    ("Garrett Nussmeier", "Kansas City Chiefs"),
    ("Aidan O'Connell", "Las Vegas Raiders"),
    ("Cole Payton", "Philadelphia Eagles"),
    ("Michael Penix Jr.", "Atlanta Falcons"),
    ("Kenny Pickett", "Carolina Panthers"),
    ("Dak Prescott", "Dallas Cowboys"),
    ("Brock Purdy", "San Francisco 49ers"),
    ("Spencer Rattler", "New Orleans Saints"),
    ("Anthony Richardson Sr.", "Indianapolis Colts"),
    ("Aaron Rodgers", "Pittsburgh Steelers"),
    ("Kurtis Rourke", "San Francisco 49ers"),
    ("Mason Rudolph", "Pittsburgh Steelers"),
    ("Cooper Rush", "Atlanta Falcons"),
    ("Shedeur Sanders", "Cleveland Browns"),
    ("Tyler Shough", "New Orleans Saints"),
    ("Ty Simpson", "Los Angeles Rams"),
    ("Geno Smith", "New York Jets"),
    ("Matthew Stafford", "Los Angeles Rams"),
    ("Jarrett Stidham", "Denver Broncos"),
    ("Jack Strand", "Atlanta Falcons"),
    ("C.J. Stroud", "Houston Texans"),
    ("Tua Tagovailoa", "Atlanta Falcons"),
    ("Tyrod Taylor", "Green Bay Packers"),
    ("Mitchell Trubisky", "Tennessee Titans"),
    ("DJ Uiagalelei", "Los Angeles Chargers"),
    ("Cam Ward", "Tennessee Titans"),
    ("Deshaun Watson", "Cleveland Browns"),
    ("Carson Wentz", "Minnesota Vikings"),
    ("Caleb Williams", "Chicago Bears"),
    ("Malik Willis", "Miami Dolphins"),
    ("Zach Wilson", "New Orleans Saints"),
    ("Jameis Winston", "New York Giants"),
    ("Bryce Young", "Carolina Panthers"),
)


def seed_teams_and_quarterbacks(apps, schema_editor):
    Team = apps.get_model("polls", "Team")
    Quarterback = apps.get_model("polls", "Quarterback")
    db = schema_editor.connection.alias

    teams = {}
    for name, abbreviation in TEAMS:
        team, _ = Team.objects.using(db).get_or_create(
            name=name, defaults={"abbreviation": abbreviation}
        )
        teams[name] = team

    for name, team_name in QUARTERBACKS:
        Quarterback.objects.using(db).get_or_create(
            name=name, defaults={"team": teams[team_name]}
        )


class Migration(migrations.Migration):
    dependencies = [("polls", "0002_approveddiscorduser")]  # noqa: RUF012

    # Reverse does not remove records: QBs may have votes, and teams/QBs may have
    # been edited after installation. Re-applying only inserts missing rows.
    operations = [  # noqa: RUF012
        migrations.RunPython(seed_teams_and_quarterbacks, migrations.RunPython.noop)
    ]
