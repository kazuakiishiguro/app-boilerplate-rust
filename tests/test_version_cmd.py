import tomli
from application_client.boilerplate_command_sender import BoilerplateCommandSender
from application_client.boilerplate_response_unpacker import unpack_get_version_response

# In this test we check the behavior of the device when asked to provide the app version
def test_version(backend):
    client = BoilerplateCommandSender(backend)
    rapdu = client.show_message()
    assert rapdu.status == 0x9000
