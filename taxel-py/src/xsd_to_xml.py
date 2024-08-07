from typing import Any, Dict
import pytest
from xmlschema import XMLSchema10, XMLSchemaValidationError
import json
import xml.etree.ElementTree as ET


def load_schema(schema_path: str) -> XMLSchema10:
    """Load and deserialize schema from xsd file"""

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')

    return schema


def load_data(data_path: str) -> Any:
    """Load and deserialize input data from json file"""
    with open(data_path, 'r') as json_file:
        json_data = json_file.read()
    data = json.loads(json_data)
    print("json data: ", json_data)

    return data


def load_xml(xml_path: str):
    """Load and deserialize xml file"""
    xml = ET.parse(xml_path)

    return xml


def generate_xml(schema: XMLSchema10, data: Any, target_namespace: str | None, namespaces: dict[str, str] | None) -> ET.ElementTree:
    print("Generate xml from schema")
    print("Target namespace: ", schema.target_namespace)
    print("Default namespace: ", schema.default_namespace)

    if target_namespace is not None and namespaces is None:
        namespaces = {target_namespace: schema.target_namespace}
    elif target_namespace is not None and namespaces is not None:
        namespaces[target_namespace] = schema.target_namespace

    if namespaces is None:
        xml_data = schema.encode(
            data, validation='strict')
    else:
        xml_data = schema.encode(
            data, validation='strict', preserve_root=True, namespaces=namespaces)

    if isinstance(xml_data, ET.Element):
        xml_tree = ET.ElementTree(xml_data)
    else:
        raise TypeError("Encoded data is not an XML Element")

    return xml_tree


def validate_xml(schema: XMLSchema10, xml_tree: ET.ElementTree):
    print("Validate xml agains schema")
    print("Target namespace: ", schema.target_namespace)
    print("Default namespace: ", schema.default_namespace)

    schema.validate(xml_tree)

    print("Validation successful")


@pytest.mark.unit
def test_validate_xml_simple():
    schema_path = '../test_data/simple/schema.xsd'
    output_path = '../test_data/simple/output.xml'

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)
    validate_xml(schema, xml)


@pytest.mark.unit
def test_generate_xml_simple():
    schema_path = '../test_data/simple/schema.xsd'
    input_path = '../test_data/simple/input.json'
    output_path = '../test_data/simple/output.xml'

    schema = load_schema(schema_path)
    data = load_data(input_path)
    xml = generate_xml(schema, data, None, None)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml


@pytest.mark.unit
def test_validate_xml_substitution_group():
    schema_path = '../test_data/substitution_group/schema1.xsd'
    output_path = '../test_data/substitution_group/output.xml'

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)
    validate_xml(schema, xml)


@pytest.mark.unit
def test_generate_xml_substitution_group():
    schema_path = '../test_data/substitution_group/schema1.xsd'
    input_path = '../test_data/substitution_group/input.json'
    output_path = '../test_data/substitution_group/output.xml'
    target_namespace = 'ns1'
    namespaces = {
        "ns2": "http://www.example.org/schema2"
    }

    schema = load_schema(schema_path)

    substitution_groups = schema.substitution_groups.as_dict()
    print("substitution groups: ", substitution_groups)
    actual_name = list(
        substitution_groups[r'abstractItem'])[0].name
    expected_name = r'{http://www.example.org/schema2}concreteItem'
    assert actual_name == expected_name

    data = load_data(input_path)
    xml = generate_xml(schema, data, target_namespace, namespaces)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml


@pytest.mark.unit
def test_validate_xml_elster():
    schema_path = '../test_data/schema/elster/elster11_bisNH_extern.xsd'
    output_path = '../test_data/elster/output.xml'

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)
    validate_xml(schema, xml)


@pytest.mark.unit
def test_generate_xml_elster():
    schema_path = '../test_data/schema/elster/elster11_bisNH_extern.xsd'
    input_path = '../test_data/elster/input.json'
    output_path = '../test_data/elster/output.xml'
    target_namespace = 'elster'
    namespaces = {
        "xs": "http://www.w3.org/2001/XMLSchema"
    }

    schema = load_schema(schema_path)
    data = load_data(input_path)
    xml = generate_xml(schema, data, target_namespace, namespaces)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml


@pytest.mark.unit
def test_validate_xml_ebilanz():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    output_path = '../test_data/ebilanz/output.xml'

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)

    validate_xml(schema, xml)


# TODO: Check iso4217:EUR for xbrli:measure
@pytest.mark.unit
def test_generate_xml_ebilanz():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    input_path = '../test_data/ebilanz/input.json'
    output_path = '../test_data/ebilanz/output.xml'
    target_namespace = 'ebilanz'
    namespaces = {
        "xsi": "http://www.w3.org/2001/XMLSchema-instance",
        "xlink": "http://www.w3.org/1999/xlink",
        "xhtml": "http://www.w3.org/1999/xhtml",
        "xbrli": "http://www.xbrl.org/2003/instance",
        "xbrldi": "http://xbrl.org/2006/xbrldi",
        "link": "http://www.xbrl.org/2003/linkbase",
        "iso4217": "http://www.xbrl.org/2003/iso4217",
    }

    schema = load_schema(schema_path)
    data = load_data(input_path)
    xml = generate_xml(schema, data, target_namespace, namespaces)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml


@pytest.mark.unit
def test_validate_xml_ebilanz_gcd():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    output_path = "../test_data/taxonomy/v6.6/de-gcd/output.xml"

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)
    validate_xml(schema, xml)


@pytest.mark.unit
def test_generate_xml_ebilanz_gcd():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    input_path = "../test_data/taxonomy/v6.6/de-gcd/input.json"
    output_path = "../test_data/taxonomy/v6.6/de-gcd/output.xml"
    target_namespace = "ebilanz"
    namespaces = {
        "xs": "http://www.w3.org/2001/XMLSchema",
        "xsi": "http://www.w3.org/2001/XMLSchema-instance",
        "xlink": "http://www.w3.org/1999/xlink",
        "xhtml": "http://www.w3.org/1999/xhtml",
        "xbrli": "http://www.xbrl.org/2003/instance",
        "xbrldi": "http://xbrl.org/2006/xbrldi",
        "link": "http://www.xbrl.org/2003/linkbase",
        "iso4217": "http://www.xbrl.org/2003/iso4217",
        "hgbref": "http://www.xbrl.de/taxonomies/de-ref-2010-02-19",
        "de-hgbrole": "http://www.xbrl.de/taxonomies/hgbrole-2022-05-02",
        # taxonomy v6.6 for Global Common Document (GCD)
        "gcd-shell": "http://www.xbrl.de/taxonomies/de-gcd-2022-05-02/shell",
        "de-gcd": "http://www.xbrl.de/taxonomies/de-gcd-2022-05-02",
        # taxonomy v6.6 for Generally Accepted Accounting Principles (GAAP)
        "de-gaap-ci": "http://www.xbrl.de/taxonomies/de-gaap-ci-2022-05-02"
    }

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')

    # TODO: Fix import of "de-gcd-2022-05-02-shell.xsd" in "xbrl-instance-2003-12-31.xsd"
    # schema.import_schema("http://www.xbrl.de/taxonomies/de-gcd-2022-05-02/shell",
    #                      "../test_data/schema/taxonomy/v6.6/de-gcd-2022-05-02/de-gcd-2022-05-02-shell.xsd")

    data = load_data(input_path)
    xml = generate_xml(schema, data, target_namespace, namespaces)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml


@pytest.mark.unit
def test_validate_xml_ebilanz_gaap_ci():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    output_path = "../test_data/taxonomy/v6.6/de-gaap-ci/output.xml"

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')
    xml = load_xml(output_path)
    validate_xml(schema, xml)


@pytest.mark.unit
def test_generate_xml_ebilanz_gaap_ci():
    schema_path = '../test_data/schema/ebilanz/ebilanz_000002.xsd'
    input_path = "../test_data/taxonomy/v6.6/de-gaap-ci/input.json"
    output_path = "../test_data/taxonomy/v6.6/de-gaap-ci/output.xml"
    target_namespace = "ebilanz"
    namespaces = {
        "xs": "http://www.w3.org/2001/XMLSchema",
        "xsi": "http://www.w3.org/2001/XMLSchema-instance",
        "xlink": "http://www.w3.org/1999/xlink",
        "xhtml": "http://www.w3.org/1999/xhtml",
        "xbrli": "http://www.xbrl.org/2003/instance",
        "xbrldt": "http://xbrl.org/2005/xbrldt",
        "xbrldi": "http://xbrl.org/2006/xbrldi",
        "link": "http://www.xbrl.org/2003/linkbase",
        "iso4217": "http://www.xbrl.org/2003/iso4217",
        "hgbref": "http://www.xbrl.de/taxonomies/de-ref-2010-02-19",
        "de-hgbrole": "http://www.xbrl.de/taxonomies/hgbrole-2022-05-02",
        # taxonomy v6.6 for Global Common Document (GCD)
        "gcd-shell": "http://www.xbrl.de/taxonomies/de-gcd-2022-05-02/shell",
        "de-gcd": "http://www.xbrl.de/taxonomies/de-gcd-2022-05-02",
        # taxonomy v6.6 for Generally Accepted Accounting Principles (GAAP)
        "gaap-shell-microbilg": "http://www.xbrl.de/taxonomies/de-gaap-ci-2022-05-02/shell-microbilg",
        "de-gaap-ci": "http://www.xbrl.de/taxonomies/de-gaap-ci-2022-05-02"
    }

    schema = XMLSchema10(schema_path, loglevel=20, validation='strict')

    data = load_data(input_path)
    xml = generate_xml(schema, data, target_namespace, namespaces)
    root = xml.getroot()
    actual_xml = ET.tostring(root, encoding="UTF-8", xml_declaration=True)

    with open(output_path, 'r', encoding="UTF-8") as file:
        expected_xml = file.read()

    assert actual_xml.decode("UTF-8") == expected_xml
