"""DB query spans; opentelemetry-instrumentation-django only traces the request."""

from django.db.backends.signals import connection_created
from opentelemetry import trace
from opentelemetry.trace import SpanKind

_tracer = trace.get_tracer(__name__)


def _trace_query(execute, sql, params, many, context):
    connection = context["connection"]
    operation = sql.split(None, 1)[0].upper() if sql else "QUERY"
    # Params are left off the span since they can carry user data.
    with _tracer.start_as_current_span(
        operation,
        kind=SpanKind.CLIENT,
        attributes={
            "db.system.name": connection.vendor,
            "db.operation.name": operation,
            "db.query.text": sql,
        },
    ):
        return execute(sql, params, many, context)


def _add_wrapper(sender, connection, **kwargs):
    # Fires on every reconnect of the same per-thread wrapper object.
    if _trace_query not in connection.execute_wrappers:
        connection.execute_wrappers.append(_trace_query)


def instrument_db() -> None:
    connection_created.connect(_add_wrapper)
