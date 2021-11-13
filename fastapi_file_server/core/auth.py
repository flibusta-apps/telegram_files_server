from fastapi.security import APIKeyHeader
from fastapi.security.utils import get_authorization_scheme_param


default_security = APIKeyHeader(name="Authorization")
