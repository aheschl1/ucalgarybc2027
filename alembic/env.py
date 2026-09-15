from logging.config import fileConfig

from sqlalchemy import create_engine, make_url, pool

from alembic import context
from api.settings import settings

config = context.config
if config.config_file_name is not None:
    fileConfig(config.config_file_name)

# SQLAlchemy is used only to hand alembic a connection; psycopg 3 is its driver.
url = make_url(settings.database_url).set(drivername="postgresql+psycopg")


def run_migrations_offline() -> None:
    context.configure(url=url, literal_binds=True, dialect_opts={"paramstyle": "named"})
    with context.begin_transaction():
        context.run_migrations()


def run_migrations_online() -> None:
    engine = create_engine(url, poolclass=pool.NullPool)
    with engine.connect() as connection:
        context.configure(connection=connection)
        with context.begin_transaction():
            context.run_migrations()


if context.is_offline_mode():
    run_migrations_offline()
else:
    run_migrations_online()
